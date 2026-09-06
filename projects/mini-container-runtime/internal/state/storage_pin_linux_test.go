//go:build linux

package state

import (
	"errors"
	"os"
	"path/filepath"
	"testing"
	"time"
)

func TestStorePinsContainerDirectoryAcrossPathReplacement(t *testing.T) {
	root := filepath.Join(t.TempDir(), "state")
	st, err := Open(root)
	if err != nil {
		t.Fatal(err)
	}

	containerPath := filepath.Join(root, "containers")
	originalContainers := filepath.Join(root, "containers-original")
	if err := os.Rename(containerPath, originalContainers); err != nil {
		t.Fatal(err)
	}
	outside := t.TempDir()
	if err := os.Symlink(outside, containerPath); err != nil {
		t.Fatal(err)
	}

	c := &Container{ID: "pinned-container", Status: StatusStopped, CreatedAt: time.Now()}
	if err := st.Save(c); err != nil {
		t.Fatalf("Save through pinned container dir: %v", err)
	}
	if _, err := os.Stat(filepath.Join(originalContainers, c.ID+".json")); err != nil {
		t.Fatalf("state was not written to original container directory: %v", err)
	}
	if _, err := os.Stat(filepath.Join(outside, c.ID+".json")); !errors.Is(err, os.ErrNotExist) {
		t.Fatalf("state escaped to replacement container directory: %v", err)
	}

	got, err := st.Get(c.ID)
	if err != nil {
		t.Fatalf("Get through pinned container dir: %v", err)
	}
	if got.ID != c.ID || got.Revision != c.Revision {
		t.Fatalf("Get returned %+v, want ID=%s revision=%d", got, c.ID, c.Revision)
	}
	listed, err := st.List()
	if err != nil {
		t.Fatalf("List through pinned container dir: %v", err)
	}
	if len(listed) != 1 || listed[0].ID != c.ID {
		t.Fatalf("List returned %+v, want only %s", listed, c.ID)
	}

	if err := st.Delete(c.ID); err != nil {
		t.Fatalf("Delete through pinned container dir: %v", err)
	}
	if _, err := os.Stat(filepath.Join(originalContainers, c.ID+".json")); !errors.Is(err, os.ErrNotExist) {
		t.Fatalf("Delete did not remove original state: %v", err)
	}
}

func TestStorePinsImageDirectoryAcrossPathReplacement(t *testing.T) {
	root := filepath.Join(t.TempDir(), "state")
	st, err := Open(root)
	if err != nil {
		t.Fatal(err)
	}

	imagePath := filepath.Join(root, "images")
	originalImages := filepath.Join(root, "images-original")
	if err := os.Rename(imagePath, originalImages); err != nil {
		t.Fatal(err)
	}
	outside := t.TempDir()
	if err := os.Symlink(outside, imagePath); err != nil {
		t.Fatal(err)
	}

	img := &Image{ID: "img-pinned", Name: "pinned:latest", RootFS: "/rootfs", LoadedAt: time.Now()}
	if err := st.SaveImage(img); err != nil {
		t.Fatalf("SaveImage through pinned image dir: %v", err)
	}
	outsideEntries, err := os.ReadDir(outside)
	if err != nil {
		t.Fatal(err)
	}
	if len(outsideEntries) != 0 {
		t.Fatalf("image metadata escaped to replacement directory: %v", outsideEntries)
	}
	originalEntries, err := os.ReadDir(originalImages)
	if err != nil {
		t.Fatal(err)
	}
	if len(originalEntries) == 0 {
		t.Fatal("image metadata was not written to original image directory")
	}

	got, err := st.GetImage(img.Name)
	if err != nil {
		t.Fatalf("GetImage through pinned image dir: %v", err)
	}
	if got.ID != img.ID || got.Name != img.Name {
		t.Fatalf("GetImage returned %+v", got)
	}
	listed, err := st.ListImages()
	if err != nil {
		t.Fatalf("ListImages through pinned image dir: %v", err)
	}
	if len(listed) != 1 || listed[0].ID != img.ID {
		t.Fatalf("ListImages returned %+v", listed)
	}
	if _, err := st.DeleteImage(img.Name); err != nil {
		t.Fatalf("DeleteImage through pinned image dir: %v", err)
	}
}

func TestStorePinsRootGenerationInternally(t *testing.T) {
	parent := t.TempDir()
	root := filepath.Join(parent, "state")
	st, err := Open(root)
	if err != nil {
		t.Fatal(err)
	}
	if st.Dir() != root {
		t.Fatalf("Store.Dir()=%q, want configured durable path %q", st.Dir(), root)
	}

	originalRoot := filepath.Join(parent, "state-original")
	if err := os.Rename(root, originalRoot); err != nil {
		t.Fatal(err)
	}
	outside := t.TempDir()
	if err := os.Symlink(outside, root); err != nil {
		t.Fatal(err)
	}

	c := &Container{ID: "root-pinned", Status: StatusStopped, CreatedAt: time.Now()}
	if err := st.Save(c); err != nil {
		t.Fatalf("Save after root replacement: %v", err)
	}
	if _, err := os.Stat(filepath.Join(originalRoot, "containers", c.ID+".json")); err != nil {
		t.Fatalf("container state missing from pinned root: %v", err)
	}
	if _, err := os.Stat(filepath.Join(outside, "containers", c.ID+".json")); !errors.Is(err, os.ErrNotExist) {
		t.Fatalf("container state escaped through replaced root: %v", err)
	}

	if _, err := Open(root); err == nil {
		t.Fatal("new Store.Open accepted symlinked replacement root")
	}
}
