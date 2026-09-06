//go:build linux

package image

import (
	"archive/tar"
	"bytes"
	"os"
	"path/filepath"
	"testing"
	"time"

	"golang.org/x/sys/unix"
)

func TestWriteRegularSecurePinsParentAcrossSymlinkReplacement(t *testing.T) {
	dest := t.TempDir()
	outside := t.TempDir()
	parent := filepath.Join(dest, "etc")
	if err := os.Mkdir(parent, 0755); err != nil { t.Fatal(err) }
	target := filepath.Join(parent, "passwd")
	hdr := &tar.Header{Name: "etc/passwd", Typeflag: tar.TypeReg, Mode: 0644, Size: 5}
	err := writeRegularSecureWithHook(target, dest, hdr, bytes.NewBufferString("safe\n"), func() {
		moved := filepath.Join(dest, "etc-pinned")
		if err := os.Rename(parent, moved); err != nil { t.Fatalf("rename parent: %v", err) }
		if err := os.Symlink(outside, parent); err != nil { t.Fatalf("replace parent with symlink: %v", err) }
	})
	if err != nil { t.Fatalf("secure regular write: %v", err) }
	if _, err := os.Stat(filepath.Join(outside, "passwd")); !os.IsNotExist(err) { t.Fatalf("outside target was created or stat failed: %v", err) }
	got, err := os.ReadFile(filepath.Join(dest, "etc-pinned", "passwd"))
	if err != nil { t.Fatalf("read pinned-parent output: %v", err) }
	if string(got) != "safe\n" { t.Fatalf("pinned-parent output = %q", got) }
}

func TestWriteRegularSecureRejectsSymlinkParent(t *testing.T) {
	dest := t.TempDir()
	outside := t.TempDir()
	if err := os.Symlink(outside, filepath.Join(dest, "etc")); err != nil { t.Fatal(err) }
	target := filepath.Join(dest, "etc", "passwd")
	hdr := &tar.Header{Name: "etc/passwd", Typeflag: tar.TypeReg, Mode: 0644, Size: 5}
	if err := writeRegularSecure(target, dest, hdr, bytes.NewBufferString("safe\n")); err == nil { t.Fatal("secure regular write accepted symlink parent") }
	if _, err := os.Stat(filepath.Join(outside, "passwd")); !os.IsNotExist(err) { t.Fatalf("outside target was created or stat failed: %v", err) }
}

func TestWriteRegularSecureRestoresModeAfterUmask(t *testing.T) {
	dest := t.TempDir()
	target := filepath.Join(dest, "bin", "tool")
	hdr := &tar.Header{Name: "bin/tool", Typeflag: tar.TypeReg, Mode: 0755, Size: 2}

	oldMask := unix.Umask(0077)
	defer unix.Umask(oldMask)

	if err := writeRegularSecure(target, dest, hdr, bytes.NewBufferString("x\n")); err != nil {
		t.Fatalf("secure regular write: %v", err)
	}
	info, err := os.Stat(target)
	if err != nil { t.Fatal(err) }
	if got := info.Mode().Perm(); got != 0755 {
		t.Fatalf("mode after restrictive umask = %04o, want 0755", got)
	}
}

func TestWriteRegularSecureRestoresMtimeByFD(t *testing.T) {
	dest := t.TempDir()
	target := filepath.Join(dest, "usr", "share", "stamp")
	want := time.Date(2020, time.January, 2, 3, 4, 5, 123000000, time.UTC)
	hdr := &tar.Header{Name: "usr/share/stamp", Typeflag: tar.TypeReg, Mode: 0644, Size: 5, ModTime: want}

	if err := writeRegularSecure(target, dest, hdr, bytes.NewBufferString("stamp")); err != nil {
		t.Fatalf("secure regular write: %v", err)
	}
	info, err := os.Stat(target)
	if err != nil { t.Fatal(err) }
	if got := info.ModTime(); !got.Equal(want) {
		t.Fatalf("mtime = %s, want %s", got, want)
	}
}
