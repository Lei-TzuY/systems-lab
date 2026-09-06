//go:build linux

package image

import (
	"archive/tar"
	"bytes"
	"os"
	"path/filepath"
	"testing"
)

func TestRemoveWhiteoutSecurePinsParentAcrossPathReplacement(t *testing.T) {
	root := t.TempDir()
	parent := filepath.Join(root, "layer")
	if err := os.MkdirAll(filepath.Join(parent, "victim", "nested"), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(parent, "victim", "nested", "old"), []byte("old"), 0o644); err != nil {
		t.Fatal(err)
	}

	outside := t.TempDir()
	if err := os.MkdirAll(filepath.Join(outside, "victim", "nested"), 0o755); err != nil {
		t.Fatal(err)
	}
	outsideSentinel := filepath.Join(outside, "victim", "nested", "keep")
	if err := os.WriteFile(outsideSentinel, []byte("keep"), 0o644); err != nil {
		t.Fatal(err)
	}

	moved := filepath.Join(root, "pinned-layer")
	target := filepath.Join(parent, "victim")
	err := removeWhiteoutSecureWithHook(target, root, func() {
		if err := os.Rename(parent, moved); err != nil {
			t.Fatal(err)
		}
		if err := os.Symlink(outside, parent); err != nil {
			t.Fatal(err)
		}
	})
	if err != nil {
		t.Fatalf("secure whiteout: %v", err)
	}
	if _, err := os.Lstat(filepath.Join(moved, "victim")); !os.IsNotExist(err) {
		t.Fatalf("pinned victim still exists: %v", err)
	}
	if got, err := os.ReadFile(outsideSentinel); err != nil || string(got) != "keep" {
		t.Fatalf("outside tree changed: data=%q err=%v", got, err)
	}
}

func TestOpaqueWhiteoutPinsDirectoryAcrossPathReplacement(t *testing.T) {
	root := t.TempDir()
	dir := filepath.Join(root, "opaque")
	if err := os.MkdirAll(filepath.Join(dir, "sub"), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(dir, "sub", "old"), []byte("old"), 0o644); err != nil {
		t.Fatal(err)
	}

	outside := t.TempDir()
	outsideSentinel := filepath.Join(outside, "keep")
	if err := os.WriteFile(outsideSentinel, []byte("keep"), 0o644); err != nil {
		t.Fatal(err)
	}

	moved := filepath.Join(root, "pinned-opaque")
	err := clearOpaqueWhiteoutSecureWithHook(dir, root, func() {
		if err := os.Rename(dir, moved); err != nil {
			t.Fatal(err)
		}
		if err := os.Symlink(outside, dir); err != nil {
			t.Fatal(err)
		}
	})
	if err != nil {
		t.Fatalf("secure opaque whiteout: %v", err)
	}
	entries, err := os.ReadDir(moved)
	if err != nil {
		t.Fatal(err)
	}
	if len(entries) != 0 {
		t.Fatalf("pinned opaque directory still has %d entries", len(entries))
	}
	if got, err := os.ReadFile(outsideSentinel); err != nil || string(got) != "keep" {
		t.Fatalf("outside tree changed: data=%q err=%v", got, err)
	}
}

func TestApplyTarEntryDoesNotMutateThroughInRootSymlinkParent(t *testing.T) {
	root := t.TempDir()
	realDir := filepath.Join(root, "real")
	if err := os.Mkdir(realDir, 0o755); err != nil {
		t.Fatal(err)
	}
	alias := filepath.Join(root, "alias")
	if err := os.Symlink(realDir, alias); err != nil {
		t.Fatal(err)
	}

	target := filepath.Join(alias, "new-parent", "payload")
	hdr := &tar.Header{Name: "alias/new-parent/payload", Typeflag: tar.TypeReg, Mode: 0o644, Size: 1}
	if err := applyTarEntry(target, hdr, bytes.NewReader([]byte("x")), root); err == nil {
		t.Fatal("symlink parent unexpectedly accepted")
	}
	if _, err := os.Lstat(filepath.Join(realDir, "new-parent")); !os.IsNotExist(err) {
		t.Fatalf("pathname preflight mutated through symlink: %v", err)
	}
}
