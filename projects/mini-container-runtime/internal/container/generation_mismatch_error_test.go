package container

import (
	"strings"
	"testing"
	"time"

	"minicontainer/internal/state"
)

func TestFinalizeStoppedGenerationReportsOwnedAndFinalizedGenerationOnMismatch(t *testing.T) {
	st, err := state.Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	defer st.Close()

	current := &state.Container{
		ID:           "ctr-finalize-mismatch",
		Status:       state.StatusRunning,
		PID:          2222,
		PIDStartTime: 20,
		CreatedAt:    time.Now(),
	}
	persistOwnedGeneration(t, st, current)

	stale := &state.Container{
		ID:           current.ID,
		Status:       state.StatusRunning,
		PID:          1111,
		PIDStartTime: 10,
		CreatedAt:    current.CreatedAt,
	}
	cleanupCalls := 0
	changed, err := finalizeStoppedGenerationWithCleanup(
		st,
		stale,
		-1,
		time.Now(),
		func(string, int, uint64) error {
			cleanupCalls++
			return nil
		},
	)
	if changed {
		t.Fatal("stale finalizer changed the replacement generation")
	}
	if err == nil {
		t.Fatal("generation ownership mismatch unexpectedly succeeded")
	}
	if cleanupCalls != 0 {
		t.Fatalf("mismatched ownership cleanup calls=%d, want 0", cleanupCalls)
	}
	text := err.Error()
	if !strings.Contains(text, "belongs to process 2222/20") || !strings.Contains(text, "not finalized generation 1111/10") {
		t.Fatalf("mismatch error=%q, want both owned and finalized generations", text)
	}
}
