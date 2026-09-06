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

func TestCreateTarSymlinkSecureRestoresLinkMetadataWithoutTouchingTarget(t *testing.T) {
	root := t.TempDir()
	targetPath := filepath.Join(root, "payload")
	if err := os.WriteFile(targetPath, []byte("payload"), 0644); err != nil {
		t.Fatal(err)
	}
	targetTime := time.Unix(1_600_000_000, 0)
	if err := os.Chtimes(targetPath, targetTime, targetTime); err != nil {
		t.Fatal(err)
	}

	linkPath := filepath.Join(root, "link")
	linkTime := time.Unix(1_700_000_123, 456_000_000)
	hdr := &tar.Header{
		Name:     "link",
		Typeflag: tar.TypeSymlink,
		Linkname: "payload",
		Uid:      os.Geteuid(),
		Gid:      os.Getegid(),
		ModTime:  linkTime,
	}
	if err := createTarSymlinkSecure(linkPath, root, hdr); err != nil {
		t.Fatalf("create tar symlink: %v", err)
	}

	info, err := os.Lstat(linkPath)
	if err != nil {
		t.Fatal(err)
	}
	if info.Mode()&os.ModeSymlink == 0 {
		t.Fatalf("created node mode=%v, want symlink", info.Mode())
	}
	if got := info.ModTime(); got.UnixNano() != linkTime.UnixNano() {
		t.Fatalf("symlink mtime=%v, want %v", got, linkTime)
	}
	stat, ok := info.Sys().(*syscall.Stat_t)
	if !ok {
		t.Fatalf("unexpected stat payload %T", info.Sys())
	}
	if int(stat.Uid) != os.Geteuid() || int(stat.Gid) != os.Getegid() {
		t.Fatalf("symlink ownership=%d:%d, want %d:%d", stat.Uid, stat.Gid, os.Geteuid(), os.Getegid())
	}

	targetInfo, err := os.Stat(targetPath)
	if err != nil {
		t.Fatal(err)
	}
	if got := targetInfo.ModTime(); got.UnixNano() != targetTime.UnixNano() {
		t.Fatalf("link target mtime changed to %v, want %v", got, targetTime)
	}
}

func TestCreateTarSymlinkSecureRejectsNegativeOwnership(t *testing.T) {
	root := t.TempDir()
	hdr := &tar.Header{
		Name:     "bad-link",
		Typeflag: tar.TypeSymlink,
		Linkname: "payload",
		Uid:      -1,
		Gid:      0,
	}
	if err := createTarSymlinkSecure(filepath.Join(root, "bad-link"), root, hdr); err == nil {
		t.Fatal("negative symlink ownership was accepted")
	}
}
