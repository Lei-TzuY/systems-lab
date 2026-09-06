package state

import (
	"encoding/json"
	"os"
	"path/filepath"
	"testing"
	"time"
)

func createModernStoppedForExitIdentityTest(t *testing.T, st *Store, id string, pid int, start uint64) *Container {
	t.Helper()
	if err := st.Save(&Container{ID: id, Status: StatusCreated, CreatedAt: time.Now()}); err != nil {
		t.Fatal(err)
	}
	if err := st.MarkRunning(id, pid, start, time.Now()); err != nil {
		t.Fatal(err)
	}
	changed, err := st.MarkStoppedIfIdentity(id, pid, start, 0, time.Now())
	if err != nil || !changed {
		t.Fatalf("MarkStoppedIfIdentity changed=%v err=%v", changed, err)
	}
	c, err := st.Get(id)
	if err != nil {
		t.Fatal(err)
	}
	return c
}

func TestEmbeddedExitIdentityWinsOverStaleLegacySidecar(t *testing.T) {
	st, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	const id = "embedded-wins"
	stopped := createModernStoppedForExitIdentityTest(t, st, id, 101, 1001)

	// Simulate stale upgrade debris arriving after the modern JSON commit.
	if err := st.writeExitedIdentityUnlocked(id, 202, 2002); err != nil {
		t.Fatal(err)
	}
	pid, start, current, ok, required, err := st.GetStoppedExitIdentityPolicy(id, stopped.Revision)
	if err != nil {
		t.Fatal(err)
	}
	if !current || !ok || !required || pid != 101 || start != 1001 {
		t.Fatalf("stale sidecar overrode embedded identity: pid=%d start=%d current=%v ok=%v required=%v", pid, start, current, ok, required)
	}
}

func TestMalformedEmbeddedExitIdentityNeverFallsBackToSidecar(t *testing.T) {
	st, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	const id = "embedded-corrupt"
	stopped := createModernStoppedForExitIdentityTest(t, st, id, 303, 3003)

	path := filepath.Join(st.ctrDir, id+".json")
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	var record map[string]json.RawMessage
	if err := json.Unmarshal(data, &record); err != nil {
		t.Fatal(err)
	}
	record["exit_identity"] = json.RawMessage("null")
	data, err = json.MarshalIndent(record, "", "  ")
	if err != nil {
		t.Fatal(err)
	}
	if err := atomicWriteFile(st.ctrDir, path, data); err != nil {
		t.Fatal(err)
	}
	if err := st.writeExitedIdentityUnlocked(id, 404, 4004); err != nil {
		t.Fatal(err)
	}

	if _, _, current, ok, _, err := st.GetStoppedExitIdentityPolicy(id, stopped.Revision); err == nil {
		t.Fatalf("corrupt embedded identity unexpectedly fell back: current=%v ok=%v", current, ok)
	}
}

func TestCapabilityOnlyJSONStillReadsLegacyExitSidecar(t *testing.T) {
	st, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	const id = "capability-sidecar-upgrade"
	if err := st.Save(&Container{ID: id, Status: StatusStopped, CreatedAt: time.Now()}); err != nil {
		t.Fatal(err)
	}
	legacy, err := st.Get(id)
	if err != nil {
		t.Fatal(err)
	}

	st.mu.Lock()
	if err := lockStateFile(st.lockFile); err != nil {
		st.mu.Unlock()
		t.Fatal(err)
	}
	if err := st.writeContainerRevisionWithExitPolicyUnlocked(legacy, legacy.Revision, true, nil); err != nil {
		_ = unlockStateFile(st.lockFile)
		st.mu.Unlock()
		t.Fatal(err)
	}
	_ = unlockStateFile(st.lockFile)
	st.mu.Unlock()
	if err := st.writeExitedIdentityUnlocked(id, 505, 5005); err != nil {
		t.Fatal(err)
	}

	pid, start, current, ok, required, err := st.GetStoppedExitIdentityPolicy(id, legacy.Revision)
	if err != nil {
		t.Fatal(err)
	}
	if !current || !ok || !required || pid != 505 || start != 5005 {
		t.Fatalf("upgrade compatibility lost: pid=%d start=%d current=%v ok=%v required=%v", pid, start, current, ok, required)
	}
}
