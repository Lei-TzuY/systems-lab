package container

import (
	"errors"
	"os"
	"path/filepath"
	"testing"
	"time"

	"minicontainer/internal/state"
)

func TestFinalizeStoppedGenerationDoesNotCleanupWhenStopStateCannotBeRead(t *testing.T) {
	dir := t.TempDir()
	st, err := state.Open(dir)
	if err != nil {
		t.Fatal(err)
	}
	defer st.Close()

	snapshot := &state.Container{
		ID:           "ctr-finalize-state-error",
		Status:       state.StatusRunning,
		PID:          5151,
		PIDStartTime: 6161,
		CreatedAt:    time.Now(),
	}
	ownership := persistOwnedGeneration(t, st, snapshot)

	// Corrupt only the container record after durable ownership exists. The
	// stop transition must fail while the independent ownership sidecar remains
	// readable. This reproduces the dangerous boundary where older code would
	// still invoke destructive cgroup cleanup despite no durable stopped state.
	statePath := filepath.Join(dir, "containers", snapshot.ID+".json")
	if err := os.WriteFile(statePath, []byte("{"), 0o600); err != nil {
		t.Fatal(err)
	}

	cleanupCalls := 0
	changed, err := finalizeStoppedGenerationWithCleanup(
		st,
		snapshot,
		-1,
		time.Now(),
		func(string, int, uint64) error {
			cleanupCalls++
			return nil
		},
	)
	if err == nil {
		t.Fatal("expected failed stopped-state transition")
	}
	if changed {
		t.Fatal("failed stopped-state transition reported changed")
	}
	if cleanupCalls != 0 {
		t.Fatalf("cleanup ran %d time(s) before stopped state was durable", cleanupCalls)
	}

	gotOwnership, ok, ownershipErr := st.GetCgroupOwnership(snapshot.ID)
	if ownershipErr != nil {
		t.Fatal(ownershipErr)
	}
	if !ok || gotOwnership != ownership {
		t.Fatalf("failed stop transition consumed retry proof: ownership=%+v ok=%v", gotOwnership, ok)
	}
	if errors.Is(err, os.ErrNotExist) {
		t.Fatalf("expected corruption/read error, got missing-state error: %v", err)
	}
}
