package container

import (
	"strings"
	"testing"
	"time"

	"minicontainer/internal/state"
)

func TestFinalizeStoppedGenerationRejectsTamperedCanonicalCgroupName(t *testing.T) {
	st, err := state.Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	snapshot := &state.Container{
		ID:           "ctr-tampered-owner",
		Status:       state.StatusRunning,
		PID:          7001,
		PIDStartTime: 8002,
		CreatedAt:    time.Now(),
	}
	if err := st.Save(snapshot); err != nil {
		t.Fatal(err)
	}
	const tamperedName = "minicontainer-other-generation"
	if err := st.MarkCgroupOwnedIfIdentity(snapshot.ID, snapshot.PID, snapshot.PIDStartTime, tamperedName); err != nil {
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
	if !changed {
		t.Fatal("tampered ownership must not prevent stopped-state reconciliation")
	}
	if err == nil || !strings.Contains(err.Error(), "does not match expected generation name") {
		t.Fatalf("tampered ownership error=%v", err)
	}
	if cleanupCalls != 0 {
		t.Fatalf("tampered ownership triggered cleanup %d time(s)", cleanupCalls)
	}
	ownership, ok, readErr := st.GetCgroupOwnership(snapshot.ID)
	if readErr != nil {
		t.Fatal(readErr)
	}
	if !ok || ownership.Name != tamperedName {
		t.Fatalf("tampered ownership proof was discarded: ownership=%+v ok=%v", ownership, ok)
	}
}
