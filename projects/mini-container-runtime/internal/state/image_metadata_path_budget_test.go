package state

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestLegacyImageMetadataPathBudgetBoundary(t *testing.T) {
	dir := t.TempDir()
	maxKey := strings.Repeat("x", maxLegacyImageMetadataFilenameBytes-len(".json"))
	path, ok := legacyImageMetadataPath(dir, maxKey)
	if !ok {
		t.Fatal("legacy pathname at exact component budget was rejected")
	}
	if got := len(filepath.Base(path)); got != maxLegacyImageMetadataFilenameBytes {
		t.Fatalf("legacy basename length=%d, want %d", got, maxLegacyImageMetadataFilenameBytes)
	}

	overlongKey := maxKey + "x"
	if path, ok := legacyImageMetadataPath(dir, overlongKey); ok || path != "" {
		t.Fatalf("overlong legacy pathname=(%q, %v), want empty,false", path, ok)
	}
}

func TestLongImageKeyUsesCurrentMetadataWithoutLegacyProbe(t *testing.T) {
	store, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	defer store.Close()

	key := strings.Repeat("long-image-key-", 32)
	if _, ok := legacyImageMetadataPath(store.imgDir, key); ok {
		t.Fatal("test key unexpectedly fits legacy filename budget")
	}
	img := &Image{Name: key, ID: "long-image-id", RootFS: "/rootfs"}
	if err := store.SaveImage(img); err != nil {
		t.Fatalf("SaveImage with overlong legacy alias: %v", err)
	}

	currentPath := filepath.Join(store.imgDir, imageMetadataFilename(key))
	if _, err := os.Stat(currentPath); err != nil {
		t.Fatalf("current hashed metadata: %v", err)
	}
	if got, err := store.GetImage(key); err != nil || got.ID != img.ID {
		t.Fatalf("GetImage long key: got=%+v err=%v", got, err)
	}
	images, err := store.ListImages()
	if err != nil || len(images) != 1 || images[0].Name != key {
		t.Fatalf("ListImages long key: images=%+v err=%v", images, err)
	}

	removed, err := store.DeleteImage(key)
	if err != nil || removed.ID != img.ID {
		t.Fatalf("DeleteImage long key: removed=%+v err=%v", removed, err)
	}
	if _, err := os.Lstat(currentPath); !os.IsNotExist(err) {
		t.Fatalf("current hashed metadata remains after delete: err=%v", err)
	}
}

func TestLegacyImageMetadataPathBudgetCountsBytes(t *testing.T) {
	key := strings.Repeat("界", 100)
	if len(legacyImageMetadataFilename(key)) <= maxLegacyImageMetadataFilenameBytes {
		t.Fatal("test key unexpectedly fits byte budget")
	}
	if path, ok := legacyImageMetadataPath(t.TempDir(), key); ok || path != "" {
		t.Fatalf("multibyte overlong legacy pathname=(%q, %v), want empty,false", path, ok)
	}
}
