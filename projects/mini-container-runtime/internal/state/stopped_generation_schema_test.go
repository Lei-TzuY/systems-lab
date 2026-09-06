package state

import (
	"encoding/json"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func rewriteStoppedGenerationRecordForTest(t *testing.T, st *Store, id string, mutate func(map[string]json.RawMessage)) {
	t.Helper()
	path := filepath.Join(st.ctrDir, id+".json")
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	var record map[string]json.RawMessage
	if err := json.Unmarshal(data, &record); err != nil {
		t.Fatal(err)
	}
	mutate(record)
	data, err = json.MarshalIndent(record, "", "  ")
	if err != nil {
		t.Fatal(err)
	}
	if err := atomicWriteFile(st.ctrDir, path, data); err != nil {
		t.Fatal(err)
	}
}

func TestModernStoppedGenerationPublishesSchemaVersion(t *testing.T) {
	st, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	const id = "schema-versioned"
	stopped := createModernStoppedForExitIdentityTest(t, st, id, 611, 6001)

	version, present, err := st.stoppedGenerationSchemaVersionUnlocked(id)
	if err != nil {
		t.Fatal(err)
	}
	if !present || version != currentStoppedGenerationSchemaVersion {
		t.Fatalf("schema version=%d present=%v", version, present)
	}
	pid, start, current, ok, required, err := st.GetStoppedExitIdentityPolicy(id, stopped.Revision)
	if err != nil {
		t.Fatal(err)
	}
	if !current || !ok || !required || pid != 611 || start != 6001 {
		t.Fatalf("versioned policy pid=%d start=%d current=%v ok=%v required=%v", pid, start, current, ok, required)
	}
}

func TestStoppedGenerationTeardownSnapshotIsCoherent(t *testing.T) {
	st, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	const id = "schema-snapshot"
	createModernStoppedForExitIdentityTest(t, st, id, 612, 6012)

	snapshot, err := st.readStoppedGenerationTeardownSnapshotUnlocked(id)
	if err != nil {
		t.Fatal(err)
	}
	if !snapshot.versioned || snapshot.version != currentStoppedGenerationSchemaVersion {
		t.Fatalf("version=%d versioned=%v", snapshot.version, snapshot.versioned)
	}
	if !snapshot.requirementPresent || !snapshot.required {
		t.Fatalf("requirement present=%v required=%v", snapshot.requirementPresent, snapshot.required)
	}
	if !snapshot.identityEmbedded || snapshot.identity.PID != 612 || snapshot.identity.PIDStartTime != 6012 {
		t.Fatalf("identity embedded=%v value=%+v", snapshot.identityEmbedded, snapshot.identity)
	}
}

func TestStoppedGenerationTeardownSnapshotRejectsMalformedMixedMetadata(t *testing.T) {
	st, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	const id = "schema-snapshot-malformed"
	stopped := createModernStoppedForExitIdentityTest(t, st, id, 613, 6013)
	rewriteStoppedGenerationRecordForTest(t, st, id, func(record map[string]json.RawMessage) {
		record["exit_identity_required"] = json.RawMessage(`"true"`)
	})

	_, _, current, ok, required, err := st.GetStoppedExitIdentityPolicy(id, stopped.Revision)
	if err == nil || !strings.Contains(err.Error(), "unmarshal stopped generation teardown metadata") {
		t.Fatalf("expected typed snapshot parse error, got current=%v ok=%v required=%v err=%v", current, ok, required, err)
	}
	if !current || ok || required {
		t.Fatalf("malformed mixed metadata did not fail closed: current=%v ok=%v required=%v", current, ok, required)
	}
}

func TestVersionedStoppedGenerationMissingIdentityNeverFallsBack(t *testing.T) {
	st, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	const id = "schema-missing-identity"
	stopped := createModernStoppedForExitIdentityTest(t, st, id, 622, 6002)
	rewriteStoppedGenerationRecordForTest(t, st, id, func(record map[string]json.RawMessage) {
		delete(record, "exit_identity")
	})
	if err := st.writeExitedIdentityUnlocked(id, 999, 9999); err != nil {
		t.Fatal(err)
	}

	_, _, current, ok, required, err := st.GetStoppedExitIdentityPolicy(id, stopped.Revision)
	if err == nil || !strings.Contains(err.Error(), "missing embedded exit identity") {
		t.Fatalf("expected versioned identity error, got current=%v ok=%v required=%v err=%v", current, ok, required, err)
	}
	if !current || ok || !required {
		t.Fatalf("unexpected fail-closed result current=%v ok=%v required=%v", current, ok, required)
	}
	if _, _, ok, err := st.GetExitedIdentity(id); err == nil || ok {
		t.Fatalf("GetExitedIdentity unexpectedly used legacy sidecar: ok=%v err=%v", ok, err)
	}
}

func TestUnknownStoppedGenerationSchemaFailsClosed(t *testing.T) {
	st, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	const id = "schema-future"
	stopped := createModernStoppedForExitIdentityTest(t, st, id, 633, 6003)
	rewriteStoppedGenerationRecordForTest(t, st, id, func(record map[string]json.RawMessage) {
		record["stopped_generation_schema_version"] = json.RawMessage("2")
	})

	_, _, current, ok, required, err := st.GetStoppedExitIdentityPolicy(id, stopped.Revision)
	if err == nil || !strings.Contains(err.Error(), "unsupported stopped generation schema version 2") {
		t.Fatalf("expected unsupported-version error, got current=%v ok=%v required=%v err=%v", current, ok, required, err)
	}
	if !current || ok || required {
		t.Fatalf("unknown version did not fail closed: current=%v ok=%v required=%v", current, ok, required)
	}
}

func TestExplicitZeroStoppedGenerationSchemaIsNotLegacy(t *testing.T) {
	st, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	const id = "schema-zero"
	stopped := createModernStoppedForExitIdentityTest(t, st, id, 644, 6004)
	rewriteStoppedGenerationRecordForTest(t, st, id, func(record map[string]json.RawMessage) {
		record["stopped_generation_schema_version"] = json.RawMessage("0")
		delete(record, "exit_identity")
		delete(record, "exit_identity_required")
	})
	if err := st.writeExitedIdentityUnlocked(id, 888, 8888); err != nil {
		t.Fatal(err)
	}

	_, _, current, ok, _, err := st.GetStoppedExitIdentityPolicy(id, stopped.Revision)
	if err == nil || !strings.Contains(err.Error(), "invalid stopped generation schema version 0") {
		t.Fatalf("explicit zero unexpectedly treated as legacy: current=%v ok=%v err=%v", current, ok, err)
	}
	if !current || ok {
		t.Fatalf("explicit zero did not fail closed: current=%v ok=%v", current, ok)
	}
}
