package image

import (
	"archive/tar"
	"encoding/json"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func dockerSaveManifestForLayers(t *testing.T, layers ...string) []byte {
	t.Helper()
	data, err := json.Marshal([]dockerManifest{{RepoTags: []string{"transaction:latest"}, Layers: layers}})
	if err != nil {
		t.Fatal(err)
	}
	return data
}

func dockerSaveLayerBytes(t *testing.T, name, body string) []byte {
	t.Helper()
	path := filepath.Join(t.TempDir(), "layer.tar")
	content := []byte(body)
	writeTarFileForDockerSaveTest(t, path, []tar.Header{{
		Name:     name,
		Mode:     0o644,
		Size:     int64(len(content)),
		Typeflag: tar.TypeReg,
	}}, [][]byte{content})
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	return data
}

func TestLoadDockerSaveFreshDestinationRollsBackLateLayerFailure(t *testing.T) {
	base := t.TempDir()
	first := dockerSaveLayerBytes(t, "first.txt", "first\n")
	broken := []byte("not a complete tar archive")
	manifest := dockerSaveManifestForLayers(t, "one/layer.tar", "two/layer.tar")
	savePath := filepath.Join(base, "save.tar")
	writeTarFileForDockerSaveTest(t, savePath, []tar.Header{
		{Name: "manifest.json", Mode: 0o644, Size: int64(len(manifest)), Typeflag: tar.TypeReg},
		{Name: "one/layer.tar", Mode: 0o644, Size: int64(len(first)), Typeflag: tar.TypeReg},
		{Name: "two/layer.tar", Mode: 0o644, Size: int64(len(broken)), Typeflag: tar.TypeReg},
	}, [][]byte{manifest, first, broken})

	dest := filepath.Join(base, "rootfs")
	err := LoadDockerSave(savePath, dest)
	if err == nil || !strings.Contains(err.Error(), "layer 2") {
		t.Fatalf("late corrupt layer error=%v", err)
	}
	if _, statErr := os.Lstat(dest); !os.IsNotExist(statErr) {
		t.Fatalf("fresh destination became visible after failed load: %v", statErr)
	}
	matches, globErr := filepath.Glob(filepath.Join(base, ".rootfs.load-*"))
	if globErr != nil {
		t.Fatal(globErr)
	}
	if len(matches) != 0 {
		t.Fatalf("failed load left staging directories: %v", matches)
	}
}

func TestLoadDockerSavePreflightsAllMembersBeforeExistingDestinationMutation(t *testing.T) {
	skipDockerSaveSymlinkTest(t)
	base := t.TempDir()
	first := dockerSaveLayerBytes(t, "first.txt", "first\n")
	externalLayer := filepath.Join(base, "outside-layer.tar")
	writeExternalLayerTar(t, externalLayer)
	manifest := dockerSaveManifestForLayers(t, "one/layer.tar", "two/layer.tar")
	savePath := filepath.Join(base, "save.tar")
	writeTarFileForDockerSaveTest(t, savePath, []tar.Header{
		{Name: "manifest.json", Mode: 0o644, Size: int64(len(manifest)), Typeflag: tar.TypeReg},
		{Name: "one/layer.tar", Mode: 0o644, Size: int64(len(first)), Typeflag: tar.TypeReg},
		{Name: "two/layer.tar", Typeflag: tar.TypeSymlink, Linkname: externalLayer, Mode: 0o777},
	}, [][]byte{manifest, first, nil})

	dest := filepath.Join(base, "existing")
	if err := os.MkdirAll(dest, 0o755); err != nil {
		t.Fatal(err)
	}
	sentinel := filepath.Join(dest, "sentinel.txt")
	if err := os.WriteFile(sentinel, []byte("original\n"), 0o600); err != nil {
		t.Fatal(err)
	}

	err := LoadDockerSave(savePath, dest)
	if err == nil || !strings.Contains(err.Error(), "layer 2") {
		t.Fatalf("invalid later member error=%v", err)
	}
	if _, statErr := os.Stat(filepath.Join(dest, "first.txt")); !os.IsNotExist(statErr) {
		t.Fatalf("first layer mutated existing destination before later member preflight: %v", statErr)
	}
	data, readErr := os.ReadFile(sentinel)
	if readErr != nil || string(data) != "original\n" {
		t.Fatalf("existing destination changed before preflight completed: data=%q err=%v", data, readErr)
	}
}

func TestLoadDockerSaveExistingDestinationKeepsOverlaySemantics(t *testing.T) {
	base := t.TempDir()
	layer := dockerSaveLayerBytes(t, "new.txt", "new\n")
	manifest := dockerSaveManifestForLayers(t, "layer/layer.tar")
	savePath := filepath.Join(base, "save.tar")
	writeTarFileForDockerSaveTest(t, savePath, []tar.Header{
		{Name: "manifest.json", Mode: 0o644, Size: int64(len(manifest)), Typeflag: tar.TypeReg},
		{Name: "layer/layer.tar", Mode: 0o644, Size: int64(len(layer)), Typeflag: tar.TypeReg},
	}, [][]byte{manifest, layer})

	dest := filepath.Join(base, "existing")
	if err := os.MkdirAll(dest, 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(dest, "keep.txt"), []byte("keep\n"), 0o600); err != nil {
		t.Fatal(err)
	}
	if err := LoadDockerSave(savePath, dest); err != nil {
		t.Fatalf("load into existing destination: %v", err)
	}
	if data, err := os.ReadFile(filepath.Join(dest, "keep.txt")); err != nil || string(data) != "keep\n" {
		t.Fatalf("existing file changed: data=%q err=%v", data, err)
	}
	if data, err := os.ReadFile(filepath.Join(dest, "new.txt")); err != nil || string(data) != "new\n" {
		t.Fatalf("new layer file missing: data=%q err=%v", data, err)
	}
}
