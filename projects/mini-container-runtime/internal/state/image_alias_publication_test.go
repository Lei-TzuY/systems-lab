package state

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestSaveImageRejectsConflictingRootFSAliasBeforePublication(t *testing.T) {
	store, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}

	first := &Image{Name: "repo:first", ID: "shared-id", RootFS: "/root-a"}
	conflict := &Image{Name: "repo:second", ID: first.ID, RootFS: "/root-b"}
	if err := store.SaveImage(first); err != nil {
		t.Fatalf("SaveImage first: %v", err)
	}
	if err := store.SaveImage(conflict); err == nil || !strings.Contains(err.Error(), "inconsistent image alias publication") {
		t.Fatalf("SaveImage conflict error=%v", err)
	}

	conflictPath := filepath.Join(store.imgDir, imageMetadataFilename(conflict.Name))
	if _, err := os.Lstat(conflictPath); !os.IsNotExist(err) {
		t.Fatalf("conflicting alias was published: err=%v", err)
	}
	got, err := store.GetImage(first.Name)
	if err != nil {
		t.Fatalf("GetImage first: %v", err)
	}
	if got.RootFS != first.RootFS {
		t.Fatalf("first RootFS=%q, want %q", got.RootFS, first.RootFS)
	}
	images, err := store.ListImages()
	if err != nil || len(images) != 1 {
		t.Fatalf("ListImages after rejection: len=%d err=%v", len(images), err)
	}
}

func TestSaveImageAllowsAliasesWithSameIDAndRootFS(t *testing.T) {
	store, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}

	first := &Image{Name: "repo:first", ID: "shared-id", RootFS: "/shared-root"}
	second := &Image{Name: "repo:second", ID: first.ID, RootFS: first.RootFS}
	for _, img := range []*Image{first, second} {
		if err := store.SaveImage(img); err != nil {
			t.Fatalf("SaveImage(%q): %v", img.Name, err)
		}
	}
	got, err := store.GetImage(first.ID)
	if err != nil {
		t.Fatalf("GetImage shared ID: %v", err)
	}
	if got.ID != first.ID || got.RootFS != first.RootFS {
		t.Fatalf("GetImage shared ID=%+v", got)
	}
	images, err := store.ListImages()
	if err != nil || len(images) != 2 {
		t.Fatalf("ListImages aliases: len=%d err=%v", len(images), err)
	}
}

func TestSaveImageRejectsConflictingAliasUpdateAndPreservesDurableState(t *testing.T) {
	store, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}

	first := &Image{Name: "repo:first", ID: "shared-id", RootFS: "/shared-root"}
	second := &Image{Name: "repo:second", ID: first.ID, RootFS: first.RootFS}
	if err := store.SaveImage(first); err != nil {
		t.Fatalf("SaveImage first: %v", err)
	}
	if err := store.SaveImage(second); err != nil {
		t.Fatalf("SaveImage second: %v", err)
	}

	updated := *second
	updated.RootFS = "/different-root"
	if err := store.SaveImage(&updated); err == nil || !strings.Contains(err.Error(), "different-root") {
		t.Fatalf("SaveImage conflicting update error=%v", err)
	}

	got, err := store.GetImage(second.Name)
	if err != nil {
		t.Fatalf("GetImage second after rejected update: %v", err)
	}
	if got.RootFS != second.RootFS {
		t.Fatalf("durable RootFS=%q, want preserved %q", got.RootFS, second.RootFS)
	}
	if _, err := store.GetImage(first.ID); err != nil {
		t.Fatalf("shared ID should remain resolvable after rejected update: %v", err)
	}
}
