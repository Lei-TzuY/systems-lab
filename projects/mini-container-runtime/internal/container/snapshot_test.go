package container

import (
	"os"
	"path/filepath"
	"testing"
	"time"

	"minicontainer/internal/state"
)

func TestContainerSnapshot(t *testing.T) {
	tmpDir := t.TempDir()
	st, err := state.Open(tmpDir)
	if err != nil {
		t.Fatalf("Open state store error: %v", err)
	}

	rootFS := filepath.Join(tmpDir, "rootfs")
	_ = os.MkdirAll(rootFS, 0755)
	_ = os.WriteFile(filepath.Join(rootFS, "data.txt"), []byte("snapshot data"), 0644)

	c := &state.Container{
		ID:        "ctr-snap-1",
		Status:    state.StatusStopped,
		RootFS:    rootFS,
		CreatedAt: time.Now(),
	}
	_ = st.Save(c)

	snap, err := CreateSnapshot(st, c.ID, "v1")
	if err != nil {
		t.Fatalf("CreateSnapshot error: %v", err)
	}

	if snap.Name != "v1" {
		t.Fatalf("Snapshot Name = %s, want v1", snap.Name)
	}

	if err := RestoreSnapshot(st, c.ID, "v1"); err != nil {
		t.Fatalf("RestoreSnapshot error: %v", err)
	}
}
