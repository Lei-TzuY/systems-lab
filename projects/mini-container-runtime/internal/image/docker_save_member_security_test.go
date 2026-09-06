package image

import (
	"archive/tar"
	"encoding/json"
	"os"
	"path/filepath"
	"runtime"
	"strings"
	"testing"
)

func writeTarFileForDockerSaveTest(t *testing.T, path string, entries []tar.Header, bodies [][]byte) {
	t.Helper()
	f, err := os.Create(path)
	if err != nil {
		t.Fatal(err)
	}
	tw := tar.NewWriter(f)
	for i := range entries {
		h := entries[i]
		if err := tw.WriteHeader(&h); err != nil {
			_ = tw.Close()
			_ = f.Close()
			t.Fatal(err)
		}
		if i < len(bodies) && len(bodies[i]) != 0 {
			if _, err := tw.Write(bodies[i]); err != nil {
				_ = tw.Close()
				_ = f.Close()
				t.Fatal(err)
			}
		}
	}
	if err := tw.Close(); err != nil {
		_ = f.Close()
		t.Fatal(err)
	}
	if err := f.Close(); err != nil {
		t.Fatal(err)
	}
}

func dockerSaveManifestBytes(t *testing.T, layer string) []byte {
	t.Helper()
	data, err := json.Marshal([]dockerManifest{{RepoTags: []string{"test:latest"}, Layers: []string{layer}}})
	if err != nil {
		t.Fatal(err)
	}
	return data
}

func writeExternalLayerTar(t *testing.T, path string) {
	t.Helper()
	body := []byte("host-controlled\n")
	writeTarFileForDockerSaveTest(t, path, []tar.Header{{
		Name:     "leak.txt",
		Mode:     0o644,
		Size:     int64(len(body)),
		Typeflag: tar.TypeReg,
	}}, [][]byte{body})
}

func skipDockerSaveSymlinkTest(t *testing.T) {
	t.Helper()
	if runtime.GOOS == "windows" {
		t.Skip("creating archive symlinks on Windows may require elevated privileges")
	}
}

func TestLoadDockerSaveReadsRegularMembers(t *testing.T) {
	base := t.TempDir()
	layerPath := filepath.Join(base, "layer.tar")
	writeExternalLayerTar(t, layerPath)
	layerBytes, err := os.ReadFile(layerPath)
	if err != nil {
		t.Fatal(err)
	}
	manifest := dockerSaveManifestBytes(t, "layer/layer.tar")
	savePath := filepath.Join(base, "save.tar")
	writeTarFileForDockerSaveTest(t, savePath, []tar.Header{
		{Name: "manifest.json", Mode: 0o644, Size: int64(len(manifest)), Typeflag: tar.TypeReg},
		{Name: "layer/layer.tar", Mode: 0o644, Size: int64(len(layerBytes)), Typeflag: tar.TypeReg},
	}, [][]byte{manifest, layerBytes})

	dest := filepath.Join(base, "rootfs")
	if err := LoadDockerSave(savePath, dest); err != nil {
		t.Fatalf("LoadDockerSave regular members: %v", err)
	}
	data, err := os.ReadFile(filepath.Join(dest, "leak.txt"))
	if err != nil || string(data) != "host-controlled\n" {
		t.Fatalf("regular layer content=%q err=%v", data, err)
	}
}

func TestLoadDockerSaveRejectsSymlinkManifest(t *testing.T) {
	skipDockerSaveSymlinkTest(t)
	base := t.TempDir()
	externalManifest := filepath.Join(base, "outside-manifest.json")
	if err := os.WriteFile(externalManifest, dockerSaveManifestBytes(t, "layer/layer.tar"), 0o600); err != nil {
		t.Fatal(err)
	}
	savePath := filepath.Join(base, "save.tar")
	writeTarFileForDockerSaveTest(t, savePath, []tar.Header{{
		Name:     "manifest.json",
		Typeflag: tar.TypeSymlink,
		Linkname: externalManifest,
		Mode:     0o777,
	}}, nil)

	err := LoadDockerSave(savePath, filepath.Join(base, "rootfs"))
	if err == nil || !strings.Contains(err.Error(), "manifest.json") {
		t.Fatalf("symlink manifest error=%v", err)
	}
}

func TestLoadDockerSaveRejectsSymlinkLayerFile(t *testing.T) {
	skipDockerSaveSymlinkTest(t)
	base := t.TempDir()
	externalLayer := filepath.Join(base, "outside-layer.tar")
	writeExternalLayerTar(t, externalLayer)
	manifest := dockerSaveManifestBytes(t, "layer/layer.tar")
	savePath := filepath.Join(base, "save.tar")
	writeTarFileForDockerSaveTest(t, savePath, []tar.Header{
		{Name: "manifest.json", Mode: 0o644, Size: int64(len(manifest)), Typeflag: tar.TypeReg},
		{Name: "layer/layer.tar", Typeflag: tar.TypeSymlink, Linkname: externalLayer, Mode: 0o777},
	}, [][]byte{manifest, nil})

	dest := filepath.Join(base, "rootfs")
	err := LoadDockerSave(savePath, dest)
	if err == nil || !strings.Contains(err.Error(), "layer 1") {
		t.Fatalf("symlink layer error=%v", err)
	}
	if _, statErr := os.Stat(filepath.Join(dest, "leak.txt")); !os.IsNotExist(statErr) {
		t.Fatalf("external layer content was applied through symlink: %v", statErr)
	}
}

func TestLoadDockerSaveRejectsSymlinkLayerAncestor(t *testing.T) {
	skipDockerSaveSymlinkTest(t)
	base := t.TempDir()
	externalDir := filepath.Join(base, "outside")
	if err := os.MkdirAll(externalDir, 0o755); err != nil {
		t.Fatal(err)
	}
	writeExternalLayerTar(t, filepath.Join(externalDir, "layer.tar"))
	manifest := dockerSaveManifestBytes(t, "alias/layer.tar")
	savePath := filepath.Join(base, "save.tar")
	writeTarFileForDockerSaveTest(t, savePath, []tar.Header{
		{Name: "manifest.json", Mode: 0o644, Size: int64(len(manifest)), Typeflag: tar.TypeReg},
		{Name: "alias", Typeflag: tar.TypeSymlink, Linkname: externalDir, Mode: 0o777},
	}, [][]byte{manifest, nil})

	dest := filepath.Join(base, "rootfs")
	err := LoadDockerSave(savePath, dest)
	if err == nil || !strings.Contains(err.Error(), "layer 1") {
		t.Fatalf("symlink layer ancestor error=%v", err)
	}
	if _, statErr := os.Stat(filepath.Join(dest, "leak.txt")); !os.IsNotExist(statErr) {
		t.Fatalf("external layer content was applied through symlink ancestor: %v", statErr)
	}
}
