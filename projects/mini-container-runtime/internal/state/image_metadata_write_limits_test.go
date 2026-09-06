package state

import (
	"os"
	"strings"
	"testing"
)

func TestSaveImageRejectsOversizedSerializedMetadataBeforePublication(t *testing.T) {
	store, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	defer store.Close()

	// Backslashes expand during JSON encoding, so a caller-owned payload well
	// below 4 MiB can still serialize beyond the authoritative read limit.
	img := &Image{
		Name: "oversized-image",
		Env:  []string{strings.Repeat("\\", int(maxStateFileBytes/2)+1)},
	}
	if err := store.SaveImage(img); err == nil || !strings.Contains(err.Error(), "size limit") {
		t.Fatalf("expected serialized-size rejection, got %v", err)
	}

	entries, err := os.ReadDir(store.imgDir)
	if err != nil {
		t.Fatalf("ReadDir(images): %v", err)
	}
	if len(entries) != 0 {
		t.Fatalf("oversized initial SaveImage mutated image directory: %v", entries)
	}
}

func TestOversizedImageUpdatePreservesDurableMetadata(t *testing.T) {
	store, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	defer store.Close()

	img := &Image{Name: "preserve-image", Env: []string{"ORIGINAL=1"}}
	if err := store.SaveImage(img); err != nil {
		t.Fatalf("initial SaveImage: %v", err)
	}

	img.Env = []string{strings.Repeat("x", int(maxStateFileBytes))}
	if err := store.SaveImage(img); err == nil || !strings.Contains(err.Error(), "size limit") {
		t.Fatalf("expected oversized update rejection, got %v", err)
	}

	persisted, err := store.GetImage(img.Name)
	if err != nil {
		t.Fatalf("GetImage after rejected update: %v", err)
	}
	if len(persisted.Env) != 1 || persisted.Env[0] != "ORIGINAL=1" {
		t.Fatalf("rejected update changed durable Env: %#v", persisted.Env)
	}
}

func TestSaveImageMetadataHelperRejectsOversizedData(t *testing.T) {
	store, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	defer store.Close()

	img := &Image{Name: "helper-boundary"}
	data := make([]byte, maxStateFileBytes+1)
	if err := store.saveImageMetadataUnlocked(img, data); err == nil || !strings.Contains(err.Error(), "size limit") {
		t.Fatalf("expected helper size-limit rejection, got %v", err)
	}
}
