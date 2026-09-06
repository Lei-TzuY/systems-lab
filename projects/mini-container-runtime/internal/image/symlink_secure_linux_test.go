//go:build linux

package image

import (
	"os"
	"path/filepath"
	"testing"
)

func TestCreateSymlinkSecurePinsParentAcrossSymlinkReplacement(t *testing.T) {
	dest := t.TempDir()
	outside := t.TempDir()
	parent := filepath.Join(dest, "etc")
	if err := os.Mkdir(parent, 0755); err != nil {
		t.Fatal(err)
	}
	target := filepath.Join(parent, "resolv.conf")

	err := createSymlinkSecureWithHook(target, dest, "../run/resolv.conf", func() {
		moved := filepath.Join(dest, "etc-pinned")
		if err := os.Rename(parent, moved); err != nil {
			t.Fatalf("rename parent: %v", err)
		}
		if err := os.Symlink(outside, parent); err != nil {
			t.Fatalf("replace parent with symlink: %v", err)
		}
	})
	if err != nil {
		t.Fatalf("secure symlink create: %v", err)
	}

	if _, err := os.Lstat(filepath.Join(outside, "resolv.conf")); !os.IsNotExist(err) {
		t.Fatalf("outside symlink was created or lstat failed: %v", err)
	}
	got, err := os.Readlink(filepath.Join(dest, "etc-pinned", "resolv.conf"))
	if err != nil {
		t.Fatalf("read pinned-parent symlink: %v", err)
	}
	if got != "../run/resolv.conf" {
		t.Fatalf("pinned-parent symlink target = %q", got)
	}
}

func TestCreateSymlinkSecureRejectsSymlinkParent(t *testing.T) {
	dest := t.TempDir()
	outside := t.TempDir()
	if err := os.Symlink(outside, filepath.Join(dest, "etc")); err != nil {
		t.Fatal(err)
	}
	target := filepath.Join(dest, "etc", "resolv.conf")
	if err := createSymlinkSecure(target, dest, "../run/resolv.conf"); err == nil {
		t.Fatal("secure symlink create accepted symlink parent")
	}
	if _, err := os.Lstat(filepath.Join(outside, "resolv.conf")); !os.IsNotExist(err) {
		t.Fatalf("outside symlink was created or lstat failed: %v", err)
	}
}

func TestCreateSymlinkSecureReplacesNonDirectoryLeaf(t *testing.T) {
	dest := t.TempDir()
	parent := filepath.Join(dest, "etc")
	if err := os.Mkdir(parent, 0755); err != nil {
		t.Fatal(err)
	}
	target := filepath.Join(parent, "resolv.conf")
	if err := os.WriteFile(target, []byte("old"), 0644); err != nil {
		t.Fatal(err)
	}
	if err := createSymlinkSecure(target, dest, "../run/resolv.conf"); err != nil {
		t.Fatalf("replace regular leaf with symlink: %v", err)
	}
	got, err := os.Readlink(target)
	if err != nil {
		t.Fatalf("read replacement symlink: %v", err)
	}
	if got != "../run/resolv.conf" {
		t.Fatalf("replacement symlink target = %q", got)
	}
}

func TestCreateSymlinkSecureRefusesDirectoryReplacement(t *testing.T) {
	dest := t.TempDir()
	target := filepath.Join(dest, "etc")
	if err := os.Mkdir(target, 0755); err != nil {
		t.Fatal(err)
	}
	if err := createSymlinkSecure(target, dest, "run/etc"); err == nil {
		t.Fatal("secure symlink create replaced directory")
	}
	info, err := os.Lstat(target)
	if err != nil {
		t.Fatalf("lstat preserved directory: %v", err)
	}
	if !info.IsDir() {
		t.Fatalf("preserved leaf mode = %v, want directory", info.Mode())
	}
}
