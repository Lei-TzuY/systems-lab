//go:build linux

package image

import (
	"os"
	"path/filepath"
	"testing"
)

func TestCreateDirectorySecurePinsParentAcrossReplacement(t *testing.T) {
	root := t.TempDir()
	parent := filepath.Join(root, "parent")
	if err := os.Mkdir(parent, 0755); err != nil {
		t.Fatal(err)
	}
	outside := t.TempDir()
	pinned := filepath.Join(root, "pinned-parent")
	target := filepath.Join(parent, "child")

	err := createDirectorySecureWithHook(target, root, 0755, func() {
		if err := os.Rename(parent, pinned); err != nil {
			t.Fatal(err)
		}
		if err := os.Symlink(outside, parent); err != nil {
			t.Fatal(err)
		}
	})
	if err != nil {
		t.Fatalf("secure directory create: %v", err)
	}
	if fi, err := os.Stat(filepath.Join(pinned, "child")); err != nil || !fi.IsDir() {
		t.Fatalf("directory not created under pinned parent: fi=%v err=%v", fi, err)
	}
	if _, err := os.Lstat(filepath.Join(outside, "child")); !os.IsNotExist(err) {
		t.Fatalf("outside directory was modified: err=%v", err)
	}
}

func TestCreateDirectorySecureReplacesSymlinkLeafOnly(t *testing.T) {
	root := t.TempDir()
	target := filepath.Join(root, "dir")
	outside := t.TempDir()
	if err := os.Symlink(outside, target); err != nil {
		t.Fatal(err)
	}
	if err := createDirectorySecure(target, root, 0755); err != nil {
		t.Fatal(err)
	}
	fi, err := os.Lstat(target)
	if err != nil {
		t.Fatal(err)
	}
	if !fi.IsDir() || fi.Mode()&os.ModeSymlink != 0 {
		t.Fatalf("target mode=%v, want real directory", fi.Mode())
	}
}

func TestCreateDirectorySecurePreservesRegularLeaf(t *testing.T) {
	root := t.TempDir()
	target := filepath.Join(root, "dir")
	if err := os.WriteFile(target, []byte("keep"), 0644); err != nil {
		t.Fatal(err)
	}
	if err := createDirectorySecure(target, root, 0755); err == nil {
		t.Fatal("regular leaf was replaced by directory")
	}
	data, err := os.ReadFile(target)
	if err != nil {
		t.Fatal(err)
	}
	if string(data) != "keep" {
		t.Fatalf("regular leaf changed: %q", data)
	}
}
