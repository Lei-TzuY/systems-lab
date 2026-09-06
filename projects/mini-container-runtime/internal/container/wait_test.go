package container

import (
	"testing"
	"time"

	"minicontainer/internal/state"
)

func TestWaitContainer(t *testing.T) {
	tmpDir := t.TempDir()
	st, err := state.Open(tmpDir)
	if err != nil {
		t.Fatalf("Open state store error: %v", err)
	}

	c := &state.Container{
		ID:        "ctr-wt-1",
		Status:    state.StatusStopped,
		ExitCode:  0,
		CreatedAt: time.Now(),
	}
	_ = st.Save(c)

	exitCode, err := WaitContainer(st, c.ID)
	if err != nil {
		t.Fatalf("WaitContainer error: %v", err)
	}
	if exitCode != 0 {
		t.Fatalf("ExitCode = %d, want 0", exitCode)
	}
}
