package registry

import (
	"archive/tar"
	"compress/gzip"
	"os"
	"path/filepath"
	"testing"
)

func writePullTestLayer(t *testing.T, path string, files map[string]string) {
	t.Helper()
	f, err := os.Create(path)
	if err != nil {
		t.Fatal(err)
	}
	gz := gzip.NewWriter(f)
	tw := tar.NewWriter(gz)
	for name, body := range files {
		if err := tw.WriteHeader(&tar.Header{Name: name, Mode: 0o644, Size: int64(len(body)), Typeflag: tar.TypeReg}); err != nil {
			t.Fatal(err)
		}
		if _, err := tw.Write([]byte(body)); err != nil {
			t.Fatal(err)
		}
	}
	if err := tw.Close(); err != nil {
		t.Fatal(err)
	}
	if err := gz.Close(); err != nil {
		t.Fatal(err)
	}
	if err := f.Close(); err != nil {
		t.Fatal(err)
	}
}

func TestApplyVerifiedLayersFreshDestinationIsTransactional(t *testing.T) {
	root := t.TempDir()
	layer1 := filepath.Join(root, "layer1.tar.gz")
	layer2 := filepath.Join(root, "layer2.tar.gz")
	writePullTestLayer(t, layer1, map[string]string{"first.txt": "committed too early"})
	if err := os.WriteFile(layer2, []byte("not a gzip layer"), 0o600); err != nil {
		t.Fatal(err)
	}

	dest := filepath.Join(root, "rootfs")
	if err := applyVerifiedLayers([]string{layer1, layer2}, dest); err == nil {
		t.Fatal("corrupt later layer unexpectedly succeeded")
	}
	if _, err := os.Lstat(dest); !os.IsNotExist(err) {
		t.Fatalf("fresh destination became visible after failed layer: %v", err)
	}
	matches, err := filepath.Glob(filepath.Join(root, ".rootfs.pull-*"))
	if err != nil {
		t.Fatal(err)
	}
	if len(matches) != 0 {
		t.Fatalf("staged pull residue remains after failure: %v", matches)
	}
}

func TestApplyVerifiedLayersFreshDestinationPublishesCompletedRootFS(t *testing.T) {
	root := t.TempDir()
	layer1 := filepath.Join(root, "layer1.tar.gz")
	layer2 := filepath.Join(root, "layer2.tar.gz")
	writePullTestLayer(t, layer1, map[string]string{"one.txt": "one"})
	writePullTestLayer(t, layer2, map[string]string{"two.txt": "two"})

	dest := filepath.Join(root, "rootfs")
	if err := applyVerifiedLayers([]string{layer1, layer2}, dest); err != nil {
		t.Fatal(err)
	}
	for name, want := range map[string]string{"one.txt": "one", "two.txt": "two"} {
		got, err := os.ReadFile(filepath.Join(dest, name))
		if err != nil {
			t.Fatal(err)
		}
		if string(got) != want {
			t.Fatalf("%s=%q want %q", name, got, want)
		}
	}
}

func TestApplyVerifiedLayersExistingDestinationKeepsOverlaySemantics(t *testing.T) {
	root := t.TempDir()
	dest := filepath.Join(root, "rootfs")
	if err := os.Mkdir(dest, 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(dest, "sentinel"), []byte("keep"), 0o600); err != nil {
		t.Fatal(err)
	}
	layer1 := filepath.Join(root, "layer1.tar.gz")
	layer2 := filepath.Join(root, "layer2.tar.gz")
	writePullTestLayer(t, layer1, map[string]string{"first.txt": "applied"})
	if err := os.WriteFile(layer2, []byte("broken"), 0o600); err != nil {
		t.Fatal(err)
	}

	if err := applyVerifiedLayers([]string{layer1, layer2}, dest); err == nil {
		t.Fatal("corrupt later layer unexpectedly succeeded")
	}
	if got, err := os.ReadFile(filepath.Join(dest, "first.txt")); err != nil || string(got) != "applied" {
		t.Fatalf("existing destination no longer has historical in-place semantics: %q err=%v", got, err)
	}
	if got, err := os.ReadFile(filepath.Join(dest, "sentinel")); err != nil || string(got) != "keep" {
		t.Fatalf("existing destination sentinel changed: %q err=%v", got, err)
	}
}
