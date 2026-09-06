package state

import (
	"strings"
	"testing"
)

func TestGetImageRejectsAliasesWithDifferentRootFS(t *testing.T) {
	store, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	const id = "fedcba654321"
	if err := store.SaveImage(&Image{ID: id, Name: "app:v1", RootFS: "/root-one"}); err != nil {
		t.Fatal(err)
	}
	writeCurrentImageMetadata(t, store, &Image{ID: id, Name: "app:latest", RootFS: "/root-two"})

	for _, selector := range []string{id, "fedcba"} {
		if _, err := store.GetImage(selector); err == nil || !strings.Contains(err.Error(), "different rootfs") {
			t.Fatalf("GetImage(%q) inconsistent alias error=%v", selector, err)
		}
	}

	// Exact tag selection remains well-defined even if another alias is corrupt.
	got, err := store.GetImage("app:v1")
	if err != nil {
		t.Fatalf("GetImage exact tag: %v", err)
	}
	if got.RootFS != "/root-one" {
		t.Fatalf("exact tag resolved to %+v", got)
	}
}
