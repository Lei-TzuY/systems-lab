//go:build linux

package image

import (
	"os"
	"path/filepath"
	"testing"

	"golang.org/x/sys/unix"
)

func TestExtractionRootPinsFilesystemGenerationAcrossPathReplacement(t *testing.T) {
	base := t.TempDir()
	rootPath := filepath.Join(base, "root")
	if err := os.MkdirAll(rootPath, 0o755); err != nil {
		t.Fatal(err)
	}
	root, err := openExtractionRoot(rootPath)
	if err != nil {
		t.Fatal(err)
	}
	defer root.Close()

	movedRoot := filepath.Join(base, "root-moved")
	if err := os.Rename(rootPath, movedRoot); err != nil {
		t.Fatal(err)
	}
	if err := os.MkdirAll(rootPath, 0o755); err != nil {
		t.Fatal(err)
	}

	parent, err := root.openParent(filepath.Join(rootPath, "nested", "leaf"), "test", true)
	if err != nil {
		t.Fatal(err)
	}
	defer parent.Close()
	if err := unix.Mkdirat(parent.fd, parent.leaf, 0o755); err != nil {
		t.Fatal(err)
	}

	if _, err := os.Stat(filepath.Join(movedRoot, "nested", "leaf")); err != nil {
		t.Fatalf("pinned root did not receive mutation: %v", err)
	}
	if _, err := os.Stat(filepath.Join(rootPath, "nested")); !os.IsNotExist(err) {
		t.Fatalf("replacement root was touched: err=%v", err)
	}
}

func TestExtractionParentReadOnlyTraversalDoesNotCreateMissingParents(t *testing.T) {
	rootPath := t.TempDir()
	root, err := openExtractionRoot(rootPath)
	if err != nil {
		t.Fatal(err)
	}
	defer root.Close()

	_, err = root.openParent(filepath.Join(rootPath, "missing", "leaf"), "source", false)
	if err == nil {
		t.Fatal("read-only traversal unexpectedly created a missing parent")
	}
	if _, statErr := os.Stat(filepath.Join(rootPath, "missing")); !os.IsNotExist(statErr) {
		t.Fatalf("missing parent was created: %v", statErr)
	}
}
