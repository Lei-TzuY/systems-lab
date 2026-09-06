package state

import (
	"encoding/json"
	"path/filepath"
	"strings"
	"testing"
)

func injectExactSelectorCollisionState(t *testing.T, store *Store, images ...*Image) {
	t.Helper()
	for _, img := range images {
		key, err := imageStorageKey(img)
		if err != nil {
			t.Fatalf("imageStorageKey(%+v): %v", img, err)
		}
		data, err := json.MarshalIndent(img, "", "  ")
		if err != nil {
			t.Fatalf("marshal image %+v: %v", img, err)
		}
		path := filepath.Join(store.imgDir, imageMetadataFilename(key))
		if err := atomicWriteFile(store.imgDir, path, data); err != nil {
			t.Fatalf("inject image state %+v: %v", img, err)
		}
	}
}

func TestGetImageRejectsExactNameExactIDCollision(t *testing.T) {
	store, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	injectExactSelectorCollisionState(t, store,
		&Image{ID: "111111111111", Name: "deadbeef", RootFS: "/named"},
		&Image{ID: "deadbeef", Name: "other:latest", RootFS: "/id"},
	)

	for _, get := range []struct {
		name string
		fn   func(string) (*Image, error)
	}{
		{name: "GetImage", fn: store.GetImage},
		{name: "GetImageUnlocked", fn: store.GetImageUnlocked},
	} {
		if _, err := get.fn("deadbeef"); err == nil || !strings.Contains(err.Error(), "both an image name/tag and an exact image ID") {
			t.Fatalf("%s exact namespace collision error=%v", get.name, err)
		}
	}
}

func TestDeleteImageRejectsExactNameExactIDCollisionWithoutDeleting(t *testing.T) {
	store, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	named := &Image{ID: "111111111111", Name: "deadbeef", RootFS: "/named"}
	byID := &Image{ID: "deadbeef", Name: "other:latest", RootFS: "/id"}
	injectExactSelectorCollisionState(t, store, named, byID)

	if _, err := store.DeleteImage("deadbeef"); err == nil || !strings.Contains(err.Error(), "both an image name/tag and an exact image ID") {
		t.Fatalf("DeleteImage exact namespace collision error=%v", err)
	}
	checks := []struct {
		selector string
		wantName string
	}{
		{selector: named.ID, wantName: named.Name},
		{selector: byID.Name, wantName: byID.Name},
	}
	for _, check := range checks {
		got, err := store.GetImage(check.selector)
		if err != nil {
			t.Fatalf("image %q unavailable after rejected delete: %v", check.selector, err)
		}
		if got.Name != check.wantName {
			t.Fatalf("GetImage(%q)=%+v, want name %q", check.selector, got, check.wantName)
		}
	}
}

func TestExactNameMayEqualItsOwnExactID(t *testing.T) {
	store, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	img := &Image{ID: "same", Name: "same", RootFS: "/same"}
	if err := store.SaveImage(img); err != nil {
		t.Fatalf("SaveImage self-overlap: %v", err)
	}

	got, err := store.GetImage("same")
	if err != nil {
		t.Fatalf("GetImage self-overlap: %v", err)
	}
	if got.Name != img.Name || got.ID != img.ID {
		t.Fatalf("GetImage self-overlap=%+v", got)
	}
}
