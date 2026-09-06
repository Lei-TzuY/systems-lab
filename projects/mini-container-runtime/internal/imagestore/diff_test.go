package imagestore

import (
	"os"
	"path/filepath"
	"testing"
	"time"

	"minicontainer/internal/state"
)

func TestDiffImages(t *testing.T) {
	tmpDir := t.TempDir()
	st, err := state.Open(tmpDir)
	if err != nil {
		t.Fatalf("Open state store error: %v", err)
	}

	dir1 := filepath.Join(tmpDir, "img1")
	dir2 := filepath.Join(tmpDir, "img2")
	_ = os.MkdirAll(dir1, 0755)
	_ = os.MkdirAll(dir2, 0755)
	_ = os.WriteFile(filepath.Join(dir2, "added.txt"), []byte("new file"), 0644)

	img1 := &state.Image{ID: "i1", Name: "v1", Tag: "v1", RootFS: dir1, LoadedAt: time.Now()}
	img2 := &state.Image{ID: "i2", Name: "v2", Tag: "v2", RootFS: dir2, LoadedAt: time.Now()}
	_ = st.SaveImage(img1)
	_ = st.SaveImage(img2)

	changes, err := DiffImages(st, "v1", "v2")
	if err != nil {
		t.Fatalf("DiffImages error: %v", err)
	}

	if len(changes) == 0 {
		t.Fatalf("Expected file diff changes between v1 and v2")
	}
}
