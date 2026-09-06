//go:build linux

package image

import (
	"archive/tar"
	"errors"
	"os"
	"path/filepath"
	"testing"
)

func TestCreateTarHardlinkMissingSourceDoesNotCreateDestinationParents(t *testing.T) {
	root := t.TempDir()
	targetParent := filepath.Join(root, "future", "nested")
	target := filepath.Join(targetParent, "alias")
	source := filepath.Join(root, "missing-source")
	hdr := &tar.Header{Name: "future/nested/alias", Linkname: "missing-source", Typeflag: tar.TypeLink, Mode: 0o644}

	err := createTarHardlinkSecure(target, root, source, hdr)
	if err == nil || !errors.Is(err, os.ErrNotExist) {
		t.Fatalf("missing source error=%v, want os.ErrNotExist", err)
	}
	if _, err := os.Lstat(filepath.Join(root, "future")); !os.IsNotExist(err) {
		t.Fatalf("missing-source hardlink mutated destination parents: err=%v", err)
	}
}

func TestCreateTarHardlinkMetadataConflictDoesNotCreateDestinationParents(t *testing.T) {
	root := t.TempDir()
	source := filepath.Join(root, "source")
	if err := os.WriteFile(source, []byte("payload"), 0o600); err != nil {
		t.Fatal(err)
	}
	target := filepath.Join(root, "future", "alias")
	hdr := &tar.Header{Name: "future/alias", Linkname: "source", Typeflag: tar.TypeLink, Mode: 0o644}

	if err := createTarHardlinkSecure(target, root, source, hdr); err == nil {
		t.Fatal("conflicting hardlink metadata was accepted")
	}
	if _, err := os.Lstat(filepath.Join(root, "future")); !os.IsNotExist(err) {
		t.Fatalf("rejected hardlink mutated destination parents: err=%v", err)
	}
}
