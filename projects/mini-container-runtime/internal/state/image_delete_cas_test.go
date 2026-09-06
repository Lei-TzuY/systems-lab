package state

import (
	"strings"
	"testing"
	"time"
)

func TestDeleteImageIfMatchRejectsReplacedMetadata(t *testing.T) {
	st, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	defer st.Close()

	original := &Image{
		ID:       "abc123",
		Name:     "cas:latest",
		Tag:      "latest",
		RootFS:   "/tmp/original-rootfs",
		LoadedAt: time.Now(),
	}
	if err := st.SaveImage(original); err != nil {
		t.Fatal(err)
	}
	expected, err := st.GetImage(original.Name)
	if err != nil {
		t.Fatal(err)
	}

	replacement := *original
	replacement.ID = "def456"
	replacement.RootFS = "/tmp/replacement-rootfs"
	if err := st.SaveImage(&replacement); err != nil {
		t.Fatal(err)
	}

	if _, err := st.DeleteImageIfMatch(original.Name, expected); err == nil || !strings.Contains(err.Error(), "changed after destructive preflight") {
		t.Fatalf("conditional delete error = %v", err)
	}
	got, err := st.GetImage(original.Name)
	if err != nil {
		t.Fatalf("replacement metadata was deleted: %v", err)
	}
	if got.ID != replacement.ID || got.RootFS != replacement.RootFS {
		t.Fatalf("replacement metadata changed: %+v", got)
	}
}

func TestDeleteImageIfMatchDeletesExactSnapshot(t *testing.T) {
	st, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	defer st.Close()

	img := &Image{ID: "abc123", Name: "cas-delete:latest", RootFS: "/tmp/rootfs", LoadedAt: time.Now()}
	if err := st.SaveImage(img); err != nil {
		t.Fatal(err)
	}
	expected, err := st.GetImage(img.Name)
	if err != nil {
		t.Fatal(err)
	}
	removed, err := st.DeleteImageIfMatch(img.Name, expected)
	if err != nil {
		t.Fatalf("DeleteImageIfMatch: %v", err)
	}
	if removed.ID != img.ID || removed.RootFS != img.RootFS {
		t.Fatalf("removed = %+v", removed)
	}
	if _, err := st.GetImage(img.Name); err == nil {
		t.Fatal("exact snapshot still present after delete")
	}
}
