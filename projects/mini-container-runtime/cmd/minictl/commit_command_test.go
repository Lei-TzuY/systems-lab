package main

import (
	"os"
	"path/filepath"
	"testing"
	"time"

	"minicontainer/internal/imagestore"
	"minicontainer/internal/state"
)

func TestCommitContainerImageCreatesIndependentManagedSnapshot(t *testing.T) {
	base := t.TempDir()
	st, err := state.Open(filepath.Join(base, "state"))
	if err != nil {
		t.Fatal(err)
	}
	defer st.Close()

	containerRootFS := filepath.Join(base, "container-rootfs")
	if err := os.MkdirAll(containerRootFS, 0o755); err != nil {
		t.Fatal(err)
	}
	containerFile := filepath.Join(containerRootFS, "app.txt")
	if err := os.WriteFile(containerFile, []byte("before commit\n"), 0o644); err != nil {
		t.Fatal(err)
	}

	rec := &state.Container{
		ID:        "cli-commit-container",
		Status:    state.StatusStopped,
		RootFS:    containerRootFS,
		CreatedAt: time.Now(),
	}
	if err := st.Save(rec); err != nil {
		t.Fatal(err)
	}

	img, err := commitContainerImage(st, rec.ID, "snapshot:latest")
	if err != nil {
		t.Fatalf("commitContainerImage: %v", err)
	}
	if filepath.Clean(img.RootFS) == filepath.Clean(containerRootFS) {
		t.Fatalf("committed image still aliases container rootfs %q", containerRootFS)
	}
	if img.ID == "" {
		t.Fatal("committed image has empty managed ID")
	}
	imageFile := filepath.Join(img.RootFS, "app.txt")
	if data, err := os.ReadFile(imageFile); err != nil || string(data) != "before commit\n" {
		t.Fatalf("snapshot content=%q err=%v", data, err)
	}

	if err := os.WriteFile(containerFile, []byte("after commit\n"), 0o644); err != nil {
		t.Fatal(err)
	}
	if data, err := os.ReadFile(imageFile); err != nil || string(data) != "before commit\n" {
		t.Fatalf("image changed with container rootfs: data=%q err=%v", data, err)
	}

	if _, err := imagestore.RemoveImage(st, img.Name, true); err != nil {
		t.Fatalf("remove committed image: %v", err)
	}
	if data, err := os.ReadFile(containerFile); err != nil || string(data) != "after commit\n" {
		t.Fatalf("removing image changed container rootfs: data=%q err=%v", data, err)
	}
}
