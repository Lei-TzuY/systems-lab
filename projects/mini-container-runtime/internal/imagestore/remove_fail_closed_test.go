package imagestore

import (
	"os"
	"path/filepath"
	"strings"
	"testing"

	"minicontainer/internal/state"
)

func TestRemoveImageRootFSPreflightRejectsCorruptMetadata(t *testing.T) {
	store, err := state.Open(filepath.Join(t.TempDir(), "state"))
	if err != nil {
		t.Fatal(err)
	}

	rootfs := filepath.Join(t.TempDir(), "rootfs")
	if err := os.MkdirAll(rootfs, 0o700); err != nil {
		t.Fatal(err)
	}
	sentinel := filepath.Join(rootfs, "sentinel")
	if err := os.WriteFile(sentinel, []byte("keep"), 0o600); err != nil {
		t.Fatal(err)
	}

	img := &state.Image{ID: "abcdef123456", Name: "safe:latest", RootFS: rootfs}
	if err := store.SaveImage(img); err != nil {
		t.Fatal(err)
	}

	corrupt := filepath.Join(store.Dir(), "images", "corrupt.json")
	if err := os.WriteFile(corrupt, []byte("{"), 0o600); err != nil {
		t.Fatal(err)
	}

	removed, err := RemoveImage(store, img.Name, true)
	if err == nil || !strings.Contains(err.Error(), "preflight image metadata") {
		t.Fatalf("RemoveImage corrupt-metadata error=%v", err)
	}
	if removed != nil {
		t.Fatalf("RemoveImage returned removed image despite failed preflight: %+v", removed)
	}
	if data, err := os.ReadFile(sentinel); err != nil || string(data) != "keep" {
		t.Fatalf("rootfs changed after failed preflight: data=%q err=%v", data, err)
	}

	// Removing only the corrupt fixture must reveal that the target metadata was
	// never deleted before the preflight failure.
	if err := os.Remove(corrupt); err != nil {
		t.Fatal(err)
	}
	got, err := store.GetImage(img.Name)
	if err != nil {
		t.Fatalf("target metadata disappeared after failed preflight: %v", err)
	}
	if got.ID != img.ID || got.RootFS != rootfs {
		t.Fatalf("target metadata changed after failed preflight: %+v", got)
	}
}

func TestRemoveImageRejectsNilStore(t *testing.T) {
	if _, err := RemoveImage(nil, "image:latest", true); err == nil || !strings.Contains(err.Error(), "state store is nil") {
		t.Fatalf("RemoveImage nil-store error=%v", err)
	}
}
