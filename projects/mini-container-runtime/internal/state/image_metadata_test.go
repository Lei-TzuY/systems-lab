package state

import (
	"encoding/json"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func writeLegacyImageMetadata(t *testing.T, store *Store, img *Image) string {
	t.Helper()
	key, err := imageStorageKey(img)
	if err != nil {
		t.Fatal(err)
	}
	data, err := json.MarshalIndent(img, "", "  ")
	if err != nil {
		t.Fatal(err)
	}
	path := filepath.Join(store.imgDir, legacyImageMetadataFilename(key))
	if err := os.WriteFile(path, data, 0o600); err != nil {
		t.Fatal(err)
	}
	return path
}

func writeCurrentImageMetadata(t *testing.T, store *Store, img *Image) string {
	t.Helper()
	key, err := imageStorageKey(img)
	if err != nil {
		t.Fatal(err)
	}
	data, err := json.MarshalIndent(img, "", "  ")
	if err != nil {
		t.Fatal(err)
	}
	path := filepath.Join(store.imgDir, imageMetadataFilename(key))
	if err := os.WriteFile(path, data, 0o600); err != nil {
		t.Fatal(err)
	}
	return path
}

func writeImageMetadataAt(t *testing.T, path string, img *Image) {
	t.Helper()
	data, err := json.MarshalIndent(img, "", "  ")
	if err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(path, data, 0o600); err != nil {
		t.Fatal(err)
	}
}

func TestImageMetadataFilenamesAvoidLegacySanitizerCollisions(t *testing.T) {
	store, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	first := &Image{Name: "repo/app:tag", ID: "id-first", RootFS: "/first"}
	second := &Image{Name: "repo_app_tag", ID: "id-second", RootFS: "/second"}
	if sanitizeImageFilename(first.Name) != sanitizeImageFilename(second.Name) {
		t.Fatal("test inputs no longer collide under legacy sanitizer")
	}
	if imageMetadataFilename(first.Name) == imageMetadataFilename(second.Name) {
		t.Fatal("collision-resistant filenames unexpectedly match")
	}
	if err := store.SaveImage(first); err != nil {
		t.Fatalf("SaveImage first: %v", err)
	}
	if err := store.SaveImage(second); err != nil {
		t.Fatalf("SaveImage second: %v", err)
	}

	images, err := store.ListImages()
	if err != nil {
		t.Fatalf("ListImages: %v", err)
	}
	if len(images) != 2 {
		t.Fatalf("images=%d, want 2", len(images))
	}
	for _, img := range []*Image{first, second} {
		got, err := store.GetImage(img.Name)
		if err != nil {
			t.Fatalf("GetImage(%q): %v", img.Name, err)
		}
		if got.ID != img.ID || got.RootFS != img.RootFS {
			t.Fatalf("GetImage(%q)=%+v, want ID=%s RootFS=%s", img.Name, got, img.ID, img.RootFS)
		}
		if _, err := os.Stat(filepath.Join(store.imgDir, imageMetadataFilename(img.Name))); err != nil {
			t.Fatalf("hashed metadata for %q: %v", img.Name, err)
		}
	}
}

func TestSaveImageMigratesOwnedLegacyMetadata(t *testing.T) {
	store, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	legacy := &Image{Name: "repo/app:tag", ID: "same-id", RootFS: "/old"}
	legacyPath := writeLegacyImageMetadata(t, store, legacy)
	updated := &Image{Name: legacy.Name, ID: legacy.ID, RootFS: "/new"}
	if err := store.SaveImage(updated); err != nil {
		t.Fatalf("SaveImage migration: %v", err)
	}
	if _, err := os.Lstat(legacyPath); !os.IsNotExist(err) {
		t.Fatalf("legacy metadata still exists: err=%v", err)
	}
	got, err := store.GetImage(updated.Name)
	if err != nil {
		t.Fatal(err)
	}
	if got.RootFS != "/new" {
		t.Fatalf("migrated RootFS=%q, want /new", got.RootFS)
	}
	images, err := store.ListImages()
	if err != nil || len(images) != 1 {
		t.Fatalf("ListImages after migration: len=%d err=%v", len(images), err)
	}
}

func TestSaveImagePreservesCollidingLegacyMetadataOwnedByAnotherImage(t *testing.T) {
	store, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	legacy := &Image{Name: "repo/app:tag", ID: "legacy-id", RootFS: "/legacy"}
	legacyPath := writeLegacyImageMetadata(t, store, legacy)
	modern := &Image{Name: "repo_app_tag", ID: "modern-id", RootFS: "/modern"}
	if sanitizeImageFilename(legacy.Name) != sanitizeImageFilename(modern.Name) {
		t.Fatal("test inputs no longer collide")
	}
	if err := store.SaveImage(modern); err != nil {
		t.Fatalf("SaveImage modern: %v", err)
	}
	if _, err := os.Stat(legacyPath); err != nil {
		t.Fatalf("colliding legacy metadata was removed: %v", err)
	}
	images, err := store.ListImages()
	if err != nil || len(images) != 2 {
		t.Fatalf("ListImages: len=%d err=%v", len(images), err)
	}

	removed, err := store.DeleteImage(modern.Name)
	if err != nil || removed.ID != modern.ID {
		t.Fatalf("DeleteImage modern: removed=%+v err=%v", removed, err)
	}
	if _, err := os.Stat(legacyPath); err != nil {
		t.Fatalf("deleting modern removed legacy collision: %v", err)
	}
	if got, err := store.GetImage(legacy.Name); err != nil || got.ID != legacy.ID {
		t.Fatalf("legacy image after modern delete: got=%+v err=%v", got, err)
	}
	if _, err := store.DeleteImage(legacy.Name); err != nil {
		t.Fatalf("DeleteImage legacy: %v", err)
	}
	if _, err := os.Lstat(legacyPath); !os.IsNotExist(err) {
		t.Fatalf("legacy metadata remains after owner delete: err=%v", err)
	}
}

func TestListImagesDeduplicatesIdenticalMigrationCopies(t *testing.T) {
	store, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	img := &Image{Name: "duplicate:test", ID: "same", RootFS: "/root"}
	writeLegacyImageMetadata(t, store, img)
	writeCurrentImageMetadata(t, store, img)
	images, err := store.ListImages()
	if err != nil {
		t.Fatalf("ListImages: %v", err)
	}
	if len(images) != 1 || images[0].Name != img.Name {
		t.Fatalf("deduplicated images=%+v", images)
	}
}

func TestListImagesPrefersCurrentMetadataDuringMigration(t *testing.T) {
	store, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	legacy := &Image{Name: "migration:test", ID: "same", RootFS: "/old"}
	current := &Image{Name: legacy.Name, ID: legacy.ID, RootFS: "/new"}
	writeLegacyImageMetadata(t, store, legacy)
	writeCurrentImageMetadata(t, store, current)
	images, err := store.ListImages()
	if err != nil {
		t.Fatalf("ListImages: %v", err)
	}
	if len(images) != 1 || images[0].RootFS != "/new" {
		t.Fatalf("authoritative migration image=%+v", images)
	}
}

func TestListImagesRejectsMetadataAtUnownedPath(t *testing.T) {
	store, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	img := &Image{Name: "alias:test", ID: "same", RootFS: "/root"}
	writeCurrentImageMetadata(t, store, img)
	writeImageMetadataAt(t, filepath.Join(store.imgDir, "unexpected-copy.json"), img)
	if _, err := store.ListImages(); err == nil || !strings.Contains(err.Error(), "pathname") {
		t.Fatalf("unowned pathname error=%v", err)
	}
}

func TestReadImageMetadataRejectsHashedFilenameForDifferentStorageKey(t *testing.T) {
	store, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	victimKey := "victim:test"
	attacker := &Image{Name: "attacker:test", ID: "attacker", RootFS: "/attacker"}
	path := filepath.Join(store.imgDir, imageMetadataFilename(victimKey))
	writeImageMetadataAt(t, path, attacker)
	if _, err := readImageMetadata(path); err == nil || !strings.Contains(err.Error(), "storage key") {
		t.Fatalf("mismatched embedded key error=%v", err)
	}
}

func TestImageMetadataFilenameIsFixedLength(t *testing.T) {
	short := imageMetadataFilename("x")
	long := imageMetadataFilename(strings.Repeat("very-long-image-name/", 1000))
	if len(short) != len(long) || len(short) != len("img-")+64+len(".json") {
		t.Fatalf("filename lengths short=%d long=%d", len(short), len(long))
	}
}
