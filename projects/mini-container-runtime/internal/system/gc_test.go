package system

import (
	"testing"
	"time"

	"minicontainer/internal/state"
)

func TestPruneUntil(t *testing.T) {
	dur, err := ParseUntilDuration("24h")
	if err != nil || dur != 24*time.Hour {
		t.Fatalf("ParseUntilDuration 24h failed: %v", err)
	}

	tmpDir := t.TempDir()
	st, err := state.Open(tmpDir)
	if err != nil {
		t.Fatalf("Open state store error: %v", err)
	}

	oldTime := time.Now().Add(-48 * time.Hour)
	cOld := &state.Container{
		ID:        "ctr-gc-old",
		Status:    state.StatusStopped,
		CreatedAt: oldTime,
	}
	_ = st.Save(cOld)

	cutoff := time.Now().Add(-24 * time.Hour)
	res, err := PruneUntil(st, cutoff)
	if err != nil || res.ContainersReclaimed != 1 {
		t.Fatalf("PruneUntil reclaimed = %d, want 1", res.ContainersReclaimed)
	}
}
