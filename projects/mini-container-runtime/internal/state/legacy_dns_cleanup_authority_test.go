package state

import (
	"encoding/json"
	"path/filepath"
	"strings"
	"testing"
	"time"
)

func TestHistoricalStoppedPolicyPersistsLegacyDNSCleanupAuthorizationAtSameRevision(t *testing.T) {
	st, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	defer st.Close()

	const id = "legacy-dns-authority"
	if err := st.Save(&Container{ID: id, Status: StatusStopped, CreatedAt: time.Now()}); err != nil {
		t.Fatal(err)
	}
	rewriteAsPreSchemaStoppedFixture(t, st, id)
	before, err := st.Get(id)
	if err != nil {
		t.Fatal(err)
	}

	_, _, current, ok, required, err := st.GetStoppedExitIdentityPolicy(id, before.Revision)
	if err != nil {
		t.Fatal(err)
	}
	if !current || ok || required {
		t.Fatalf("unexpected historical policy: current=%v ok=%v required=%v", current, ok, required)
	}

	after, err := st.Get(id)
	if err != nil {
		t.Fatal(err)
	}
	if after.Revision != before.Revision {
		t.Fatalf("legacy classification advanced lifecycle revision: before=%d after=%d", before.Revision, after.Revision)
	}

	data, err := readRegularStateFile(filepath.Join(st.ctrDir, id+".json"), "container state")
	if err != nil {
		t.Fatal(err)
	}
	var raw map[string]json.RawMessage
	if err := json.Unmarshal(data, &raw); err != nil {
		t.Fatal(err)
	}
	var authorized bool
	if err := json.Unmarshal(raw["legacy_dns_cleanup_authorized"], &authorized); err != nil {
		t.Fatalf("legacy DNS cleanup capability was not durably published: %v", err)
	}
	if !authorized {
		t.Fatal("legacy DNS cleanup capability is not true")
	}
	var version uint32
	if err := json.Unmarshal(raw["state_schema_version"], &version); err != nil || version != currentContainerStateSchemaVersion {
		t.Fatalf("legacy classification did not migrate writer provenance: version=%d err=%v", version, err)
	}
}

func TestHistoricalStoppedPolicyDoesNotClassifyStaleRevision(t *testing.T) {
	st, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	defer st.Close()

	const id = "legacy-dns-stale"
	if err := st.Save(&Container{ID: id, Status: StatusStopped, CreatedAt: time.Now()}); err != nil {
		t.Fatal(err)
	}
	rewriteAsPreSchemaStoppedFixture(t, st, id)
	currentState, err := st.Get(id)
	if err != nil {
		t.Fatal(err)
	}

	_, _, current, ok, required, err := st.GetStoppedExitIdentityPolicy(id, currentState.Revision+1)
	if err != nil {
		t.Fatal(err)
	}
	if current || ok || required {
		t.Fatalf("stale revision acquired legacy authority: current=%v ok=%v required=%v", current, ok, required)
	}

	data, err := readRegularStateFile(filepath.Join(st.ctrDir, id+".json"), "container state")
	if err != nil {
		t.Fatal(err)
	}
	var raw map[string]json.RawMessage
	if err := json.Unmarshal(data, &raw); err != nil {
		t.Fatal(err)
	}
	if _, exists := raw["legacy_dns_cleanup_authorized"]; exists {
		t.Fatal("stale revision durably classified the current stopped generation")
	}
	if _, exists := raw["state_schema_version"]; exists {
		t.Fatal("stale revision rewrote historical writer provenance")
	}
}

func TestHistoricalStoppedPolicyRejectsExplicitFalseLegacyAuthorization(t *testing.T) {
	st, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	defer st.Close()

	const id = "legacy-dns-false"
	if err := st.Save(&Container{ID: id, Status: StatusStopped, CreatedAt: time.Now()}); err != nil {
		t.Fatal(err)
	}
	container, err := st.Get(id)
	if err != nil {
		t.Fatal(err)
	}

	record := struct {
		*Container
		LegacyDNSCleanupAuthorized bool `json:"legacy_dns_cleanup_authorized"`
	}{Container: container, LegacyDNSCleanupAuthorized: false}
	data, err := json.MarshalIndent(&record, "", "  ")
	if err != nil {
		t.Fatal(err)
	}
	if err := atomicWriteFile(st.ctrDir, filepath.Join(st.ctrDir, id+".json"), data); err != nil {
		t.Fatal(err)
	}

	_, _, current, ok, required, err := st.GetStoppedExitIdentityPolicy(id, container.Revision)
	if err == nil || !strings.Contains(err.Error(), "invalid legacy DNS cleanup authorization: false") {
		t.Fatalf("expected explicit false authorization to fail closed, got current=%v ok=%v required=%v err=%v", current, ok, required, err)
	}
	if !current || ok || required {
		t.Fatalf("malformed legacy authorization acquired teardown authority: current=%v ok=%v required=%v", current, ok, required)
	}
}

func TestHistoricalStoppedPolicyRejectsSidecarAddedAfterLegacyAuthorization(t *testing.T) {
	st, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	defer st.Close()

	const id = "legacy-dns-sidecar-conflict"
	if err := st.Save(&Container{ID: id, Status: StatusStopped, CreatedAt: time.Now()}); err != nil {
		t.Fatal(err)
	}
	rewriteAsPreSchemaStoppedFixture(t, st, id)
	container, err := st.Get(id)
	if err != nil {
		t.Fatal(err)
	}
	if _, _, _, _, _, err := st.GetStoppedExitIdentityPolicy(id, container.Revision); err != nil {
		t.Fatal(err)
	}

	st.mu.Lock()
	if err := lockStateFile(st.lockFile); err != nil {
		st.mu.Unlock()
		t.Fatal(err)
	}
	if err := st.writeExitedIdentityUnlocked(id, 5151, 9191); err != nil {
		_ = unlockStateFile(st.lockFile)
		st.mu.Unlock()
		t.Fatal(err)
	}
	_ = unlockStateFile(st.lockFile)
	st.mu.Unlock()

	_, _, current, ok, required, err := st.GetStoppedExitIdentityPolicy(id, container.Revision)
	if err == nil || !strings.Contains(err.Error(), "conflicts with exited identity sidecar metadata") {
		t.Fatalf("expected conflicting sidecar to fail closed, got current=%v ok=%v required=%v err=%v", current, ok, required, err)
	}
	if !current || ok || required {
		t.Fatalf("conflicting sidecar broadened teardown authority: current=%v ok=%v required=%v", current, ok, required)
	}
}
