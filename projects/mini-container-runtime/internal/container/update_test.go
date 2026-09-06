package container

import (
	"testing"
	"time"

	"minicontainer/internal/state"
)

func TestUpdateContainer(t *testing.T) {
	tmpDir := t.TempDir()
	st, err := state.Open(tmpDir)
	if err != nil {
		t.Fatalf("Open state store error: %v", err)
	}

	c := &state.Container{
		ID:        "ctr-upd-1",
		Status:    state.StatusRunning,
		PID:       12345,
		CreatedAt: time.Now(),
	}
	_ = st.Save(c)

	opts := UpdateContainerOptions{
		MemoryLimit: "256MB",
		CPUQuota:    1.0,
	}

	if err := UpdateContainer(st, c.ID, opts); err != nil {
		t.Fatalf("UpdateContainer error: %v", err)
	}
}
