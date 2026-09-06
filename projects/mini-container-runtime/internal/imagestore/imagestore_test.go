package imagestore

import (
	"os"
	"path/filepath"
	"testing"
	"time"

	"minicontainer/internal/state"
)

func TestImageStoreOps(t *testing.T) {
	tmpDir := t.TempDir()
	st, err := state.Open(tmpDir)
	if err != nil {
		t.Fatalf("Open state store error: %v", err)
	}

	rootfs := filepath.Join(tmpDir, "rootfs-demo")
	if err := os.MkdirAll(filepath.Join(rootfs, "etc"), 0755); err != nil {
		t.Fatalf("Mkdir error: %v", err)
	}
	if err := os.WriteFile(filepath.Join(rootfs, "etc", "hello.txt"), []byte("hello world"), 0644); err != nil {
		t.Fatalf("Write file error: %v", err)
	}

	sz, err := CalculateDirSize(rootfs)
	if err != nil || sz == 0 {
		t.Fatalf("CalculateDirSize = %d, err: %v", sz, err)
	}

	id := GenerateImageID()
	if len(id) != 12 {
		t.Fatalf("GenerateImageID length = %d, want 12", len(id))
	}

	img := &state.Image{
		ID:         id,
		Repository: "demoapp",
		Tag:        "v1",
		Name:       "demoapp:v1",
		RootFS:     rootfs,
		Size:       sz,
		LoadedAt:   time.Now(),
	}

	if err := st.SaveImage(img); err != nil {
		t.Fatalf("SaveImage error: %v", err)
	}

	got, err := st.GetImage("demoapp:v1")
	if err != nil || got.ID != id {
		t.Fatalf("GetImage failed: %v", err)
	}

	// Test Tag
	tagged, err := TagImage(st, "demoapp:v1", "demoapp:latest")
	if err != nil || tagged.Tag != "latest" {
		t.Fatalf("TagImage error: %v", err)
	}

	imgs, err := st.ListImages()
	if err != nil || len(imgs) != 2 {
		t.Fatalf("ListImages count = %d, want 2", len(imgs))
	}

	// Remove tagged tag without deleting rootfs
	if _, err := RemoveImage(st, "demoapp:latest", true); err != nil {
		t.Fatalf("RemoveImage error: %v", err)
	}

	if _, err := os.Stat(rootfs); os.IsNotExist(err) {
		t.Fatalf("RootFS deleted prematurely while demoapp:v1 still exists")
	}

	// Remove remaining tag with rootfs removal
	if _, err := RemoveImage(st, "demoapp:v1", true); err != nil {
		t.Fatalf("RemoveImage v1 error: %v", err)
	}

	if _, err := os.Stat(rootfs); !os.IsNotExist(err) {
		t.Fatalf("RootFS should be removed after last tag is deleted")
	}
}
