package imagestore

import (
	"os"
	"path/filepath"
	"testing"
	"time"

	"minicontainer/internal/state"
)

func TestVerifyImageIntegrity(t *testing.T) {
	tmpDir := t.TempDir()
	st, err := state.Open(tmpDir)
	if err != nil {
		t.Fatalf("Open state store error: %v", err)
	}

	rootFS := filepath.Join(tmpDir, "rootfs")
	if err := os.MkdirAll(rootFS, 0755); err != nil {
		t.Fatalf("MkdirAll rootFS error: %v", err)
	}

	img := &state.Image{
		ID:       "img-v-1",
		Tag:      "verify:v1",
		Name:     "verify:v1",
		RootFS:   rootFS,
		LoadedAt: time.Now(),
	}
	_ = st.SaveImage(img)

	ok, digest, err := VerifyImageIntegrity(st, "verify:v1")
	if err != nil || !ok {
		t.Fatalf("VerifyImageIntegrity error: %v (ok=%v)", err, ok)
	}
	if digest == "" {
		t.Fatalf("Digest is empty")
	}
}
