package registry

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"minicontainer/internal/state"
)

func TestOCIPushAndManifest(t *testing.T) {
	tmpDir := t.TempDir()
	st, err := state.Open(tmpDir)
	if err != nil {
		t.Fatalf("Open state store error: %v", err)
	}

	rootfs := filepath.Join(tmpDir, "sample-rootfs")
	_ = os.MkdirAll(filepath.Join(rootfs, "etc"), 0755)
	_ = os.WriteFile(filepath.Join(rootfs, "etc", "app.conf"), []byte("mode=production"), 0644)

	img := &state.Image{
		ID:         "img-push-123",
		Repository: "my-app",
		Tag:        "v1",
		Name:       "my-app:v1",
		RootFS:     rootfs,
		LoadedAt:   time.Now(),
	}
	if err := st.SaveImage(img); err != nil {
		t.Fatalf("SaveImage error: %v", err)
	}

	outLayer := filepath.Join(tmpDir, "layer.tar.gz")
	err = PushImage(st, "my-app:v1", outLayer)
	if err != nil {
		t.Fatalf("PushImage error: %v", err)
	}

	if _, err := os.Stat(outLayer); os.IsNotExist(err) {
		t.Fatalf("Output layer archive missing")
	}

	manifestFile := outLayer + ".manifest.json"
	data, err := os.ReadFile(manifestFile)
	if err != nil || !strings.Contains(string(data), "application/vnd.oci.image.manifest.v1+json") {
		t.Fatalf("OCI Manifest invalid: %v, content:\n%s", err, string(data))
	}
}
