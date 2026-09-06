package imagestore

import (
	"os"
	"path/filepath"
	"testing"
	"time"

	"minicontainer/internal/state"
)

func TestCalculateImageSize(t *testing.T) {
	tmpDir := t.TempDir()
	st, err := state.Open(tmpDir)
	if err != nil {
		t.Fatalf("Open state store error: %v", err)
	}

	rootFS := filepath.Join(tmpDir, "rootfs")
	_ = os.MkdirAll(rootFS, 0755)
	_ = os.WriteFile(filepath.Join(rootFS, "data.bin"), []byte("1234567890"), 0644)

	img := &state.Image{
		ID:       "img-sz-1",
		Tag:      "size:v1",
		Name:     "size:v1",
		RootFS:   rootFS,
		LoadedAt: time.Now(),
	}
	_ = st.SaveImage(img)

	sz, err := CalculateImageSize(st, "size:v1")
	if err != nil {
		t.Fatalf("CalculateImageSize error: %v", err)
	}
	if sz != 10 {
		t.Fatalf("Size = %d, want 10", sz)
	}
}
