package state

import (
	"encoding/json"
	"os"
	"strings"
	"testing"
	"time"
)

func TestStoppedGenerationPolicyMigratesLegacySidecarAtSameRevision(t *testing.T) {
	st, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	defer st.Close()

	if err := st.Save(&Container{ID: "legacy-migrate", Status: StatusStopped, CreatedAt: time.Now()}); err != nil {
		t.Fatal(err)
	}
	rewriteAsPreSchemaStoppedFixture(t, st, "legacy-migrate")
	before, err := st.Get("legacy-migrate")
	if err != nil {
		t.Fatal(err)
	}

	st.mu.Lock()
	if err := lockStateFile(st.lockFile); err != nil {
		st.mu.Unlock()
		t.Fatal(err)
	}
	if err := st.writeExitedIdentityUnlocked("legacy-migrate", 4242, 777); err != nil {
		_ = unlockStateFile(st.lockFile)
		st.mu.Unlock()
		t.Fatal(err)
	}
	if err := atomicWriteFile(st.ctrDir, exitedIdentityRequiredPath(st.ctrDir, "legacy-migrate"), []byte("1\n")); err != nil {
		_ = unlockStateFile(st.lockFile)
		st.mu.Unlock()
		t.Fatal(err)
	}
	_ = unlockStateFile(st.lockFile)
	st.mu.Unlock()

	pid, start, current, ok, required, err := st.GetStoppedExitIdentityPolicy("legacy-migrate", before.Revision)
	if err != nil {
		t.Fatal(err)
	}
	if !current || !ok || !required || pid != 4242 || start != 777 {
		t.Fatalf("unexpected migrated policy: pid=%d start=%d current=%v ok=%v required=%v", pid, start, current, ok, required)
	}

	after, err := st.Get("legacy-migrate")
	if err != nil {
		t.Fatal(err)
	}
	if after.Revision != before.Revision {
		t.Fatalf("migration advanced lifecycle revision: before=%d after=%d", before.Revision, after.Revision)
	}

	st.mu.Lock()
	snapshot, err := st.readStoppedGenerationTeardownSnapshotUnlocked("legacy-migrate")
	st.mu.Unlock()
	if err != nil {
		t.Fatal(err)
	}
	if !snapshot.stateVersioned || !snapshot.versioned || snapshot.version != currentStoppedGenerationSchemaVersion || !snapshot.requirementPresent || !snapshot.required || !snapshot.identityEmbedded {
		t.Fatalf("legacy sidecar was not durably migrated: %+v", snapshot)
	}
	if snapshot.identity.PID != 4242 || snapshot.identity.PIDStartTime != 777 {
		t.Fatalf("wrong embedded identity after migration: %+v", snapshot.identity)
	}
}

func TestStoppedGenerationPolicyRejectsUnmarkedLegacyIdentitySidecar(t *testing.T) {
	st, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	defer st.Close()

	const id = "legacy-unmarked"
	if err := st.Save(&Container{ID: id, Status: StatusStopped, CreatedAt: time.Now()}); err != nil {
		t.Fatal(err)
	}
	rewriteAsPreSchemaStoppedFixture(t, st, id)
	before, err := st.Get(id)
	if err != nil {
		t.Fatal(err)
	}

	st.mu.Lock()
	if err := lockStateFile(st.lockFile); err != nil {
		st.mu.Unlock()
		t.Fatal(err)
	}
	if err := st.writeExitedIdentityUnlocked(id, 4343, 778); err != nil {
		_ = unlockStateFile(st.lockFile)
		st.mu.Unlock()
		t.Fatal(err)
	}
	_ = unlockStateFile(st.lockFile)
	st.mu.Unlock()

	_, _, current, ok, required, err := st.GetStoppedExitIdentityPolicy(id, before.Revision)
	if err == nil || !strings.Contains(err.Error(), "without required capability marker") {
		t.Fatalf("expected orphan legacy identity to fail closed, got current=%v ok=%v required=%v err=%v", current, ok, required, err)
	}
	if !current || ok || required {
		t.Fatalf("orphan legacy identity acquired teardown authority: current=%v ok=%v required=%v", current, ok, required)
	}

	st.mu.Lock()
	snapshot, snapshotErr := st.readStoppedGenerationTeardownSnapshotUnlocked(id)
	_, sidecarErr := os.Lstat(exitedIdentityPath(st.ctrDir, id))
	st.mu.Unlock()
	if snapshotErr != nil {
		t.Fatal(snapshotErr)
	}
	if snapshot.stateVersioned || snapshot.versioned || snapshot.identityEmbedded || snapshot.requirementPresent {
		t.Fatalf("orphan legacy identity mutated lifecycle metadata: %+v", snapshot)
	}
	if sidecarErr != nil {
		t.Fatalf("failed migration unexpectedly retired evidence sidecar: %v", sidecarErr)
	}
}

func TestStoppedGenerationPolicyDoesNotMigrateStaleRevision(t *testing.T) {
	st, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	defer st.Close()

	if err := st.Save(&Container{ID: "legacy-stale", Status: StatusStopped, CreatedAt: time.Now()}); err != nil {
		t.Fatal(err)
	}
	rewriteAsPreSchemaStoppedFixture(t, st, "legacy-stale")
	currentState, err := st.Get("legacy-stale")
	if err != nil {
		t.Fatal(err)
	}

	st.mu.Lock()
	if err := lockStateFile(st.lockFile); err != nil {
		st.mu.Unlock()
		t.Fatal(err)
	}
	if err := st.writeExitedIdentityUnlocked("legacy-stale", 5252, 888); err != nil {
		_ = unlockStateFile(st.lockFile)
		st.mu.Unlock()
		t.Fatal(err)
	}
	_ = unlockStateFile(st.lockFile)
	st.mu.Unlock()

	_, _, current, ok, required, err := st.GetStoppedExitIdentityPolicy("legacy-stale", currentState.Revision+1)
	if err != nil {
		t.Fatal(err)
	}
	if current || ok || required {
		t.Fatalf("stale revision acquired migration authority: current=%v ok=%v required=%v", current, ok, required)
	}

	st.mu.Lock()
	snapshot, err := st.readStoppedGenerationTeardownSnapshotUnlocked("legacy-stale")
	st.mu.Unlock()
	if err != nil {
		t.Fatal(err)
	}
	if snapshot.stateVersioned || snapshot.versioned || snapshot.identityEmbedded || snapshot.requirementPresent {
		t.Fatalf("stale revision mutated lifecycle metadata: %+v", snapshot)
	}
}

func TestStoppedGenerationPolicyMigratesPreVersionEmbeddedIdentity(t *testing.T) {
	st, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	defer st.Close()

	if err := st.Save(&Container{ID: "embedded-migrate", Status: StatusStopped, CreatedAt: time.Now()}); err != nil {
		t.Fatal(err)
	}
	container, err := st.Get("embedded-migrate")
	if err != nil {
		t.Fatal(err)
	}
	identity := exitedIdentity{PID: 6262, PIDStartTime: 999}

	st.mu.Lock()
	if err := lockStateFile(st.lockFile); err != nil {
		st.mu.Unlock()
		t.Fatal(err)
	}
	// Model the pre-version embedded format by writing current exact metadata and
	// then removing both writer and stopped-generation schema keys from the fixture.
	if err := st.writeContainerRevisionWithExitPolicyUnlocked(container, container.Revision, true, &identity); err != nil {
		_ = unlockStateFile(st.lockFile)
		st.mu.Unlock()
		t.Fatal(err)
	}
	path := st.ctrDir + "/embedded-migrate.json"
	data, err := readRegularStateFile(path, "container state")
	if err != nil {
		_ = unlockStateFile(st.lockFile)
		st.mu.Unlock()
		t.Fatal(err)
	}
	var raw map[string]any
	if err := json.Unmarshal(data, &raw); err != nil {
		_ = unlockStateFile(st.lockFile)
		st.mu.Unlock()
		t.Fatal(err)
	}
	delete(raw, "state_schema_version")
	delete(raw, "stopped_generation_schema_version")
	data, err = json.MarshalIndent(raw, "", "  ")
	if err != nil {
		_ = unlockStateFile(st.lockFile)
		st.mu.Unlock()
		t.Fatal(err)
	}
	if err := atomicWriteFile(st.ctrDir, path, data); err != nil {
		_ = unlockStateFile(st.lockFile)
		st.mu.Unlock()
		t.Fatal(err)
	}
	_ = unlockStateFile(st.lockFile)
	st.mu.Unlock()

	pid, start, current, ok, required, err := st.GetStoppedExitIdentityPolicy("embedded-migrate", container.Revision)
	if err != nil {
		t.Fatal(err)
	}
	if !current || !ok || !required || pid != identity.PID || start != identity.PIDStartTime {
		t.Fatalf("unexpected embedded migration policy: pid=%d start=%d current=%v ok=%v required=%v", pid, start, current, ok, required)
	}

	st.mu.Lock()
	snapshot, err := st.readStoppedGenerationTeardownSnapshotUnlocked("embedded-migrate")
	st.mu.Unlock()
	if err != nil {
		t.Fatal(err)
	}
	if !snapshot.stateVersioned || !snapshot.versioned || !snapshot.identityEmbedded || !snapshot.required {
		t.Fatalf("embedded pre-version record was not upgraded: %+v", snapshot)
	}
}
