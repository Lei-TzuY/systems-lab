package container

import (
	"os"
	"testing"
	"time"

	"minicontainer/internal/state"
)

func TestGetContainerThreads(t *testing.T) {
	tmpDir := t.TempDir()
	st, err := state.Open(tmpDir)
	if err != nil {
		t.Fatalf("Open state store error: %v", err)
	}

	c := &state.Container{
		ID:        "ctr-th-1",
		Status:    state.StatusRunning,
		PID:       os.Getpid(),
		CreatedAt: time.Now(),
	}
	_ = st.Save(c)

	threads, err := GetContainerThreads(st, c.ID)
	if err != nil {
		t.Fatalf("GetContainerThreads error: %v", err)
	}
	if len(threads) == 0 {
		t.Fatalf("Threads list is empty")
	}
}
