package container

import (
	"testing"
	"time"

	"minicontainer/internal/state"
)

func TestGetStatsSnapshot(t *testing.T) {
	tmpDir := t.TempDir()
	st, err := state.Open(tmpDir)
	if err != nil {
		t.Fatalf("Open state store error: %v", err)
	}

	c := &state.Container{
		ID:        "ctr-snp-1",
		Status:    state.StatusRunning,
		PID:       4321,
		CreatedAt: time.Now(),
	}
	_ = st.Save(c)

	snap, err := GetStatsSnapshot(st, c.ID)
	if err != nil {
		t.Fatalf("GetStatsSnapshot error: %v", err)
	}

	if snap.ContainerID != c.ID {
		t.Fatalf("ContainerID = %s, want %s", snap.ContainerID, c.ID)
	}
}
