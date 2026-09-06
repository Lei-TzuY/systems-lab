package container

import (
	"path/filepath"
	"testing"
	"time"

	"minicontainer/internal/state"
)

func TestDumpContainerMemory(t *testing.T) {
	tmpDir := t.TempDir()
	st, err := state.Open(tmpDir)
	if err != nil {
		t.Fatalf("Open state store error: %v", err)
	}

	c := &state.Container{
		ID:        "ctr-dump-1",
		PID:       1234,
		Status:    state.StatusRunning,
		Command:   []string{"sh"},
		CreatedAt: time.Now(),
	}
	_ = st.Save(c)

	dumpFile := filepath.Join(tmpDir, "ctr-dump-1.dump")
	info, err := DumpContainerMemory(st, c.ID, dumpFile)
	if err != nil {
		t.Fatalf("DumpContainerMemory error: %v", err)
	}

	if info.ContainerID != c.ID {
		t.Fatalf("DumpInfo ContainerID = %s, want %s", info.ContainerID, c.ID)
	}
}
