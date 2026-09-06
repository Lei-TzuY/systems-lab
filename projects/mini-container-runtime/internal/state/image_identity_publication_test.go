package state

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestSaveImageRejectsNameCollidingWithDifferentExactIDBeforePublication(t *testing.T) {
	store, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}

	existing := &Image{Name: "repo:base", ID: "sha-target", RootFS: "/root-a"}
	conflict := &Image{Name: existing.ID, ID: "different-id", RootFS: "/root-b"}
	if err := store.SaveImage(existing); err != nil {
		t.Fatalf("SaveImage existing: %v", err)
	}
	if err := store.SaveImage(conflict); err == nil || !strings.Contains(err.Error(), "ambiguous image identity publication") {
		t.Fatalf("SaveImage conflicting name error=%v", err)
	}

	conflictPath := filepath.Join(store.imgDir, imageMetadataFilename(conflict.Name))
	if _, err := os.Lstat(conflictPath); !os.IsNotExist(err) {
		t.Fatalf("conflicting image was published: err=%v", err)
	}
	got, err := store.GetImage(existing.ID)
	if err != nil {
		t.Fatalf("GetImage existing ID: %v", err)
	}
	if got.ID != existing.ID || got.Name != existing.Name {
		t.Fatalf("existing image changed after rejected publication: %+v", got)
	}
}

func TestSaveImageRejectsIDCollidingWithDifferentExactNameBeforePublication(t *testing.T) {
	store, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}

	existing := &Image{Name: "repo:claimed", ID: "existing-id", RootFS: "/root-a"}
	conflict := &Image{Name: "repo:new", ID: existing.Name, RootFS: "/root-b"}
	if err := store.SaveImage(existing); err != nil {
		t.Fatalf("SaveImage existing: %v", err)
	}
	if err := store.SaveImage(conflict); err == nil || !strings.Contains(err.Error(), "collides with exact name") {
		t.Fatalf("SaveImage conflicting ID error=%v", err)
	}

	conflictPath := filepath.Join(store.imgDir, imageMetadataFilename(conflict.Name))
	if _, err := os.Lstat(conflictPath); !os.IsNotExist(err) {
		t.Fatalf("conflicting image was published: err=%v", err)
	}
	if _, err := store.GetImage(existing.Name); err != nil {
		t.Fatalf("existing exact name should remain resolvable: %v", err)
	}
}

func TestSaveImageRejectedIdentityUpdatePreservesDurableState(t *testing.T) {
	store, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}

	claimed := &Image{Name: "repo:claimed", ID: "claimed-id", RootFS: "/root-a"}
	victim := &Image{Name: "repo:victim", ID: "victim-id", RootFS: "/root-b"}
	if err := store.SaveImage(claimed); err != nil {
		t.Fatalf("SaveImage claimed: %v", err)
	}
	if err := store.SaveImage(victim); err != nil {
		t.Fatalf("SaveImage victim: %v", err)
	}

	updated := *victim
	updated.ID = claimed.Name
	if err := store.SaveImage(&updated); err == nil || !strings.Contains(err.Error(), "ambiguous image identity publication") {
		t.Fatalf("SaveImage ambiguous update error=%v", err)
	}

	got, err := store.GetImage(victim.Name)
	if err != nil {
		t.Fatalf("GetImage victim after rejected update: %v", err)
	}
	if got.ID != victim.ID || got.RootFS != victim.RootFS {
		t.Fatalf("durable victim changed after rejected update: got=%+v want=%+v", got, victim)
	}
}

func TestSaveImageRejectsCrossRecordNameIDOverlapForSameID(t *testing.T) {
	store, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}

	first := &Image{Name: "repo:first", ID: "shared-id", RootFS: "/shared-root"}
	alias := &Image{Name: first.ID, ID: first.ID, RootFS: first.RootFS}
	if err := store.SaveImage(first); err != nil {
		t.Fatalf("SaveImage first: %v", err)
	}
	if err := store.SaveImage(alias); err == nil || !strings.Contains(err.Error(), "collides with exact ID") {
		t.Fatalf("SaveImage same-ID cross-record overlap error=%v", err)
	}

	aliasPath := filepath.Join(store.imgDir, imageMetadataFilename(alias.Name))
	if _, err := os.Lstat(aliasPath); !os.IsNotExist(err) {
		t.Fatalf("ambiguous same-ID alias was published: err=%v", err)
	}
	got, err := store.GetImage(first.ID)
	if err != nil {
		t.Fatalf("GetImage first ID after rejected alias: %v", err)
	}
	if got.Name != first.Name || got.ID != first.ID {
		t.Fatalf("existing image changed after rejected alias: %+v", got)
	}
}
