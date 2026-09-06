package state

import (
	"strings"
	"testing"
)

func saveSelectorImages(t *testing.T, store *Store, images ...*Image) {
	t.Helper()
	for _, img := range images {
		if err := store.SaveImage(img); err != nil {
			t.Fatalf("SaveImage(%+v): %v", img, err)
		}
	}
}

func TestGetImageRejectsPrefixMatchingDifferentIDs(t *testing.T) {
	store, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	saveSelectorImages(t, store,
		&Image{ID: "abc111111111", Name: "first:latest", RootFS: "/first"},
		&Image{ID: "abc222222222", Name: "second:latest", RootFS: "/second"},
	)

	if _, err := store.GetImage("abc"); err == nil || !strings.Contains(err.Error(), "ambiguous image ID prefix") {
		t.Fatalf("GetImage ambiguous prefix error=%v", err)
	}
	if _, err := store.GetImageUnlocked("abc"); err == nil || !strings.Contains(err.Error(), "ambiguous image ID prefix") {
		t.Fatalf("GetImageUnlocked ambiguous prefix error=%v", err)
	}
}

func TestGetImageAllowsAliasesOfOneIDDeterministically(t *testing.T) {
	store, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	saveSelectorImages(t, store,
		&Image{ID: "abcdef123456", Name: "zeta:latest", RootFS: "/same"},
		&Image{ID: "abcdef123456", Name: "alpha:latest", RootFS: "/same"},
	)

	got, err := store.GetImage("abcdef")
	if err != nil {
		t.Fatalf("GetImage alias ID prefix: %v", err)
	}
	if got.ID != "abcdef123456" || got.Name != "alpha:latest" {
		t.Fatalf("GetImage alias representative=%+v, want deterministic alpha alias", got)
	}
}

func TestGetImageExactNameWinsOverIDPrefix(t *testing.T) {
	store, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	saveSelectorImages(t, store,
		&Image{ID: "deadbeef1111", Name: "dead", RootFS: "/named"},
		&Image{ID: "dead22222222", Name: "other:latest", RootFS: "/prefix"},
	)

	got, err := store.GetImage("dead")
	if err != nil {
		t.Fatalf("GetImage exact name: %v", err)
	}
	if got.Name != "dead" || got.RootFS != "/named" {
		t.Fatalf("exact named selector resolved to %+v", got)
	}
}

func TestGetImageExactIDWinsOverLongerPrefixMatch(t *testing.T) {
	store, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	const exactID = "abc123"
	saveSelectorImages(t, store,
		&Image{ID: exactID, Name: "exact:latest", RootFS: "/exact"},
		&Image{ID: "abc123456789", Name: "longer:latest", RootFS: "/longer"},
	)

	got, err := store.GetImage(exactID)
	if err != nil {
		t.Fatalf("GetImage exact ID: %v", err)
	}
	if got.ID != exactID || got.RootFS != "/exact" {
		t.Fatalf("exact ID resolved to %+v", got)
	}
}

func TestDeleteImageRejectsAmbiguousPrefixWithoutDeleting(t *testing.T) {
	store, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	first := &Image{ID: "abc111111111", Name: "first:latest", RootFS: "/first"}
	second := &Image{ID: "abc222222222", Name: "second:latest", RootFS: "/second"}
	saveSelectorImages(t, store, first, second)

	if _, err := store.DeleteImage("abc"); err == nil || !strings.Contains(err.Error(), "ambiguous image ID prefix") {
		t.Fatalf("DeleteImage ambiguous prefix error=%v", err)
	}
	for _, name := range []string{first.Name, second.Name} {
		if _, err := store.GetImage(name); err != nil {
			t.Fatalf("image %q disappeared after rejected delete: %v", name, err)
		}
	}
}

func TestDeleteImageByIDRejectsMultipleAliases(t *testing.T) {
	store, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	const id = "abcdef123456"
	saveSelectorImages(t, store,
		&Image{ID: id, Name: "app:v1", RootFS: "/same"},
		&Image{ID: id, Name: "app:latest", RootFS: "/same"},
	)

	if _, err := store.DeleteImage(id); err == nil || !strings.Contains(err.Error(), "multiple tags") {
		t.Fatalf("DeleteImage shared ID error=%v", err)
	}
	if _, err := store.DeleteImage("abcdef"); err == nil || !strings.Contains(err.Error(), "multiple tags") {
		t.Fatalf("DeleteImage shared ID prefix error=%v", err)
	}
	for _, name := range []string{"app:v1", "app:latest"} {
		if _, err := store.GetImage(name); err != nil {
			t.Fatalf("alias %q disappeared after rejected ID delete: %v", name, err)
		}
	}
}

func TestDeleteImageExactIDWinsOverLongerPrefixMatch(t *testing.T) {
	store, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	const exactID = "abc123"
	saveSelectorImages(t, store,
		&Image{ID: exactID, Name: "exact:latest", RootFS: "/exact"},
		&Image{ID: "abc123456789", Name: "longer:latest", RootFS: "/longer"},
	)

	removed, err := store.DeleteImage(exactID)
	if err != nil {
		t.Fatalf("DeleteImage exact ID: %v", err)
	}
	if removed.ID != exactID {
		t.Fatalf("removed=%+v", removed)
	}
	if _, err := store.GetImage("longer:latest"); err != nil {
		t.Fatalf("longer-prefix image disappeared: %v", err)
	}
}

func TestDeleteImageExactTagRemainsSupported(t *testing.T) {
	store, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	const id = "abcdef123456"
	saveSelectorImages(t, store,
		&Image{ID: id, Name: "app:v1", Repository: "app", Tag: "v1", RootFS: "/same"},
		&Image{ID: id, Name: "app:latest", Repository: "app", Tag: "latest", RootFS: "/same"},
	)

	removed, err := store.DeleteImage("app:v1")
	if err != nil {
		t.Fatalf("DeleteImage exact tag: %v", err)
	}
	if removed.Name != "app:v1" {
		t.Fatalf("removed=%+v", removed)
	}
	if _, err := store.GetImage("app:v1"); err == nil {
		t.Fatal("deleted exact tag still resolves")
	}
	remaining, err := store.GetImage("app:latest")
	if err != nil || remaining.ID != id {
		t.Fatalf("remaining alias=%+v err=%v", remaining, err)
	}
}

func TestDeleteImageByUniqueIDStillWorks(t *testing.T) {
	store, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	img := &Image{ID: "0123456789ab", Name: "single:latest", RootFS: "/single"}
	saveSelectorImages(t, store, img)

	removed, err := store.DeleteImage("012345")
	if err != nil {
		t.Fatalf("DeleteImage unique prefix: %v", err)
	}
	if removed.Name != img.Name {
		t.Fatalf("removed=%+v", removed)
	}
	if _, err := store.GetImage(img.Name); err == nil {
		t.Fatal("unique image still resolves after delete")
	}
}
