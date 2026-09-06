package system

import (
	"testing"
	"time"

	"minicontainer/internal/state"
)

func TestSystemDFAndPrune(t *testing.T) {
	tmpDir := t.TempDir()
	st, err := state.Open(tmpDir)
	if err != nil {
		t.Fatalf("Open state store error: %v", err)
	}

	c1 := &state.Container{
		ID:        "ctr-df-1",
		Status:    state.StatusStopped,
		RootFS:    tmpDir,
		CreatedAt: time.Now(),
	}
	_ = st.Save(c1)

	df, err := CalculateDF(st)
	if err != nil || df.ContainersCount != 1 {
		t.Fatalf("CalculateDF failed: %v, count: %d", err, df.ContainersCount)
	}

	pruneRes, err := SystemPrune(st, false)
	if err != nil || pruneRes.ContainersReclaimed != 1 {
		t.Fatalf("SystemPrune containers reclaimed = %d, want 1", pruneRes.ContainersReclaimed)
	}

	ctrs, _ := st.List()
	if len(ctrs) != 0 {
		t.Fatalf("Stopped container should have been pruned")
	}
}
