//go:build linux

package image

import (
	"archive/tar"
	"os"
	"path/filepath"
	"syscall"
	"testing"
	"time"
)

func TestMakeSpecialSecurePinsParentAgainstSymlinkReplacement(t *testing.T) {
	root := t.TempDir()
	outside := t.TempDir()
	parent := filepath.Join(root, "parent")
	pinnedParent := filepath.Join(root, "parent-pinned")
	if err := os.Mkdir(parent, 0o755); err != nil {
		t.Fatal(err)
	}
	target := filepath.Join(parent, "pipe")
	hdr := &tar.Header{Typeflag: tar.TypeFifo, Mode: 0o600, Uid: os.Getuid(), Gid: os.Getgid()}

	err := makeSpecialSecureWithHook(target, root, hdr, func() {
		if err := os.Rename(parent, pinnedParent); err != nil {
			t.Fatalf("rename parent: %v", err)
		}
		if err := os.Symlink(outside, parent); err != nil {
			t.Fatalf("replace parent with outside symlink: %v", err)
		}
	})
	if err != nil {
		t.Fatalf("secure FIFO creation: %v", err)
	}

	fi, err := os.Lstat(filepath.Join(pinnedParent, "pipe"))
	if err != nil {
		t.Fatalf("pinned FIFO missing: %v", err)
	}
	if fi.Mode()&os.ModeNamedPipe == 0 {
		t.Fatalf("pinned target mode=%v, want named pipe", fi.Mode())
	}
	if _, err := os.Lstat(filepath.Join(outside, "pipe")); !os.IsNotExist(err) {
		t.Fatalf("outside target was touched: %v", err)
	}
}

func TestMakeSpecialSecureRefusesDirectoryReplacement(t *testing.T) {
	root := t.TempDir()
	target := filepath.Join(root, "keep")
	if err := os.Mkdir(target, 0o755); err != nil {
		t.Fatal(err)
	}
	hdr := &tar.Header{Typeflag: tar.TypeFifo, Mode: 0o600}
	if err := makeSpecialSecure(target, root, hdr); err == nil {
		t.Fatal("directory replacement unexpectedly succeeded")
	}
	fi, err := os.Lstat(target)
	if err != nil {
		t.Fatalf("preserved directory missing: %v", err)
	}
	if !fi.IsDir() {
		t.Fatalf("target mode=%v, want preserved directory", fi.Mode())
	}
}

func TestMakeSpecialSecureMetadataStaysBoundToPinnedInode(t *testing.T) {
	root := t.TempDir()
	target := filepath.Join(root, "pipe")
	pinned := filepath.Join(root, "pipe-pinned")
	mtime := time.Unix(978307200, 123456789)
	hdr := &tar.Header{
		Typeflag: tar.TypeFifo,
		Mode:     0o6750,
		Uid:      os.Getuid(),
		Gid:      os.Getgid(),
		ModTime:  mtime,
	}

	err := makeSpecialSecureWithHooks(target, root, hdr, nil, func() {
		if err := os.Rename(target, pinned); err != nil {
			t.Fatalf("rename pinned FIFO: %v", err)
		}
		if err := os.WriteFile(target, []byte("foreign"), 0o600); err != nil {
			t.Fatalf("create foreign replacement: %v", err)
		}
	})
	if err != nil {
		t.Fatalf("restore pinned FIFO metadata: %v", err)
	}

	fi, err := os.Lstat(pinned)
	if err != nil {
		t.Fatalf("pinned FIFO missing: %v", err)
	}
	if fi.Mode()&os.ModeNamedPipe == 0 {
		t.Fatalf("pinned mode=%v, want FIFO", fi.Mode())
	}
	if fi.Mode().Perm() != 0o750 || fi.Mode()&os.ModeSetuid == 0 || fi.Mode()&os.ModeSetgid == 0 {
		t.Fatalf("pinned mode=%v, want 06750", fi.Mode())
	}
	st, ok := fi.Sys().(*syscall.Stat_t)
	if !ok {
		t.Fatalf("unexpected stat payload %T", fi.Sys())
	}
	if int(st.Uid) != hdr.Uid || int(st.Gid) != hdr.Gid {
		t.Fatalf("pinned ownership=%d:%d, want %d:%d", st.Uid, st.Gid, hdr.Uid, hdr.Gid)
	}
	if !fi.ModTime().Equal(mtime) {
		t.Fatalf("pinned mtime=%v, want %v", fi.ModTime(), mtime)
	}

	foreign, err := os.Lstat(target)
	if err != nil {
		t.Fatalf("foreign replacement missing: %v", err)
	}
	if !foreign.Mode().IsRegular() || foreign.Mode().Perm() != 0o600 {
		t.Fatalf("foreign replacement mode changed: %v", foreign.Mode())
	}
	if foreign.ModTime().Equal(mtime) {
		t.Fatalf("foreign replacement mtime was modified to archive timestamp")
	}
}
