//go:build linux

package imagestore

import (
	"errors"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"minicontainer/internal/state"
)

func saveManagedImageForRemoval(t *testing.T, st *state.Store, root, id, name string) (*state.Image, string) {
	t.Helper()
	rootFS := filepath.Join(root, "images", id, "rootfs")
	if err := os.MkdirAll(rootFS, 0o755); err != nil {
		t.Fatal(err)
	}
	sentinel := filepath.Join(rootFS, "owned.txt")
	if err := os.WriteFile(sentinel, []byte("owned\n"), 0o600); err != nil {
		t.Fatal(err)
	}
	img := &state.Image{ID: id, Name: name, Tag: name, RootFS: rootFS}
	if err := st.SaveImage(img); err != nil {
		t.Fatal(err)
	}
	return img, sentinel
}

func TestRemoveImagePinsManagedRootFSAndPreservesSiblingArtifacts(t *testing.T) {
	root := filepath.Join(t.TempDir(), "state")
	st, err := state.Open(root)
	if err != nil {
		t.Fatal(err)
	}
	defer st.Close()

	id := strings.Repeat("a", 64)
	img, _ := saveManagedImageForRemoval(t, st, root, id, "managed:latest")
	layer := filepath.Join(root, "images", id, "layer.tar.gz")
	if err := os.WriteFile(layer, []byte("layer\n"), 0o600); err != nil {
		t.Fatal(err)
	}

	removed, err := RemoveImage(st, img.Name, true)
	if err != nil {
		t.Fatalf("RemoveImage: %v", err)
	}
	if removed.ID != id {
		t.Fatalf("removed image ID=%q, want %q", removed.ID, id)
	}
	if _, err := os.Stat(img.RootFS); !errors.Is(err, os.ErrNotExist) {
		t.Fatalf("managed rootfs still exists: %v", err)
	}
	if data, err := os.ReadFile(layer); err != nil || string(data) != "layer\n" {
		t.Fatalf("sibling layer changed: data=%q err=%v", data, err)
	}
	if _, err := st.GetImage(img.Name); err == nil {
		t.Fatal("removed image metadata still exists")
	}
}

func TestRemoveImageRejectsConfiguredStateRootReplacementBeforeMetadataDelete(t *testing.T) {
	parent := t.TempDir()
	root := filepath.Join(parent, "state")
	st, err := state.Open(root)
	if err != nil {
		t.Fatal(err)
	}
	defer st.Close()

	img, sentinel := saveManagedImageForRemoval(t, st, root, strings.Repeat("b", 64), "root-replaced:latest")
	originalRoot := filepath.Join(parent, "state-original")
	if err := os.Rename(root, originalRoot); err != nil {
		t.Fatal(err)
	}
	outside := t.TempDir()
	outsideSentinel := filepath.Join(outside, "outside.txt")
	if err := os.WriteFile(outsideSentinel, []byte("outside\n"), 0o600); err != nil {
		t.Fatal(err)
	}
	if err := os.Symlink(outside, root); err != nil {
		t.Fatal(err)
	}

	if _, err := RemoveImage(st, img.Name, true); err == nil || !strings.Contains(err.Error(), "acquire managed image storage") {
		t.Fatalf("root replacement error=%v", err)
	}
	if _, err := st.GetImage(img.Name); err != nil {
		t.Fatalf("metadata deleted after failed generation check: %v", err)
	}
	originalSentinel := filepath.Join(originalRoot, "images", img.ID, "rootfs", filepath.Base(sentinel))
	if data, err := os.ReadFile(originalSentinel); err != nil || string(data) != "owned\n" {
		t.Fatalf("original managed payload changed: data=%q err=%v", data, err)
	}
	if data, err := os.ReadFile(outsideSentinel); err != nil || string(data) != "outside\n" {
		t.Fatalf("replacement target changed: data=%q err=%v", data, err)
	}
}

func TestRemoveImageRejectsConfiguredImagesReplacementBeforeMetadataDelete(t *testing.T) {
	root := filepath.Join(t.TempDir(), "state")
	st, err := state.Open(root)
	if err != nil {
		t.Fatal(err)
	}
	defer st.Close()

	img, _ := saveManagedImageForRemoval(t, st, root, strings.Repeat("c", 64), "images-replaced:latest")
	images := filepath.Join(root, "images")
	originalImages := filepath.Join(root, "images-original")
	if err := os.Rename(images, originalImages); err != nil {
		t.Fatal(err)
	}
	outside := t.TempDir()
	outsideSentinel := filepath.Join(outside, "outside.txt")
	if err := os.WriteFile(outsideSentinel, []byte("outside\n"), 0o600); err != nil {
		t.Fatal(err)
	}
	if err := os.Symlink(outside, images); err != nil {
		t.Fatal(err)
	}

	if _, err := RemoveImage(st, img.Name, true); err == nil || !strings.Contains(err.Error(), "acquire managed image storage") {
		t.Fatalf("images replacement error=%v", err)
	}
	if _, err := st.GetImage(img.Name); err != nil {
		t.Fatalf("metadata deleted after failed images generation check: %v", err)
	}
	if data, err := os.ReadFile(filepath.Join(originalImages, img.ID, "rootfs", "owned.txt")); err != nil || string(data) != "owned\n" {
		t.Fatalf("original payload changed: data=%q err=%v", data, err)
	}
	if data, err := os.ReadFile(outsideSentinel); err != nil || string(data) != "outside\n" {
		t.Fatalf("replacement images target changed: data=%q err=%v", data, err)
	}
}

func TestRemoveImageRejectsManagedImageDirectorySymlinkBeforeMetadataDelete(t *testing.T) {
	root := filepath.Join(t.TempDir(), "state")
	st, err := state.Open(root)
	if err != nil {
		t.Fatal(err)
	}
	defer st.Close()

	img, _ := saveManagedImageForRemoval(t, st, root, strings.Repeat("d", 64), "image-link:latest")
	imageDir := filepath.Join(root, "images", img.ID)
	originalImageDir := imageDir + "-original"
	if err := os.Rename(imageDir, originalImageDir); err != nil {
		t.Fatal(err)
	}
	outside := t.TempDir()
	outsideSentinel := filepath.Join(outside, "outside.txt")
	if err := os.WriteFile(outsideSentinel, []byte("outside\n"), 0o600); err != nil {
		t.Fatal(err)
	}
	if err := os.Symlink(outside, imageDir); err != nil {
		t.Fatal(err)
	}

	if _, err := RemoveImage(st, img.Name, true); err == nil || !strings.Contains(err.Error(), "pin managed image rootfs") {
		t.Fatalf("image symlink error=%v", err)
	}
	if _, err := st.GetImage(img.Name); err != nil {
		t.Fatalf("metadata deleted after image symlink rejection: %v", err)
	}
	if data, err := os.ReadFile(filepath.Join(originalImageDir, "rootfs", "owned.txt")); err != nil || string(data) != "owned\n" {
		t.Fatalf("original payload changed: data=%q err=%v", data, err)
	}
	if data, err := os.ReadFile(outsideSentinel); err != nil || string(data) != "outside\n" {
		t.Fatalf("symlink target changed: data=%q err=%v", data, err)
	}
}

func TestRemoveImageRejectsManagedRootFSSymlinkBeforeMetadataDelete(t *testing.T) {
	root := filepath.Join(t.TempDir(), "state")
	st, err := state.Open(root)
	if err != nil {
		t.Fatal(err)
	}
	defer st.Close()

	img, _ := saveManagedImageForRemoval(t, st, root, strings.Repeat("e", 64), "rootfs-link:latest")
	rootFS := img.RootFS
	originalRootFS := rootFS + "-original"
	if err := os.Rename(rootFS, originalRootFS); err != nil {
		t.Fatal(err)
	}
	outside := t.TempDir()
	outsideSentinel := filepath.Join(outside, "outside.txt")
	if err := os.WriteFile(outsideSentinel, []byte("outside\n"), 0o600); err != nil {
		t.Fatal(err)
	}
	if err := os.Symlink(outside, rootFS); err != nil {
		t.Fatal(err)
	}

	if _, err := RemoveImage(st, img.Name, true); err == nil || !strings.Contains(err.Error(), "must be a real directory") {
		t.Fatalf("rootfs symlink error=%v", err)
	}
	if _, err := st.GetImage(img.Name); err != nil {
		t.Fatalf("metadata deleted after rootfs symlink rejection: %v", err)
	}
	if data, err := os.ReadFile(filepath.Join(originalRootFS, "owned.txt")); err != nil || string(data) != "owned\n" {
		t.Fatalf("original rootfs changed: data=%q err=%v", data, err)
	}
	if data, err := os.ReadFile(outsideSentinel); err != nil || string(data) != "outside\n" {
		t.Fatalf("rootfs symlink target changed: data=%q err=%v", data, err)
	}
}

func TestPinnedManagedRootFSRemovalNeverFollowsReplacementSymlink(t *testing.T) {
	images := filepath.Join(t.TempDir(), "images")
	id := strings.Repeat("f", 64)
	rootFS := filepath.Join(images, id, "rootfs")
	if err := os.MkdirAll(rootFS, 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(rootFS, "owned.txt"), []byte("owned\n"), 0o600); err != nil {
		t.Fatal(err)
	}

	removal, err := pinManagedImageRootFS(images, id)
	if err != nil {
		t.Fatal(err)
	}
	defer removal.Close()

	moved := rootFS + "-moved"
	if err := os.Rename(rootFS, moved); err != nil {
		t.Fatal(err)
	}
	outside := t.TempDir()
	outsideSentinel := filepath.Join(outside, "outside.txt")
	if err := os.WriteFile(outsideSentinel, []byte("outside\n"), 0o600); err != nil {
		t.Fatal(err)
	}
	if err := os.Symlink(outside, rootFS); err != nil {
		t.Fatal(err)
	}

	if err := removal.Remove(); err == nil || !strings.Contains(err.Error(), "changed filesystem identity") {
		t.Fatalf("replacement race error=%v", err)
	}
	if data, err := os.ReadFile(outsideSentinel); err != nil || string(data) != "outside\n" {
		t.Fatalf("replacement target changed: data=%q err=%v", data, err)
	}
}
