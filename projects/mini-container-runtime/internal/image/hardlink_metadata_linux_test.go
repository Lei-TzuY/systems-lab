//go:build linux

package image

import (
	"archive/tar"
	"errors"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"golang.org/x/sys/unix"
)

func TestCreateTarHardlinkSecureRejectsModeConflictBeforeDestinationMutation(t *testing.T) {
	root := t.TempDir()
	source := filepath.Join(root, "source")
	dest := filepath.Join(root, "dest")
	if err := os.WriteFile(source, []byte("source"), 0o644); err != nil {
		t.Fatal(err)
	}
	if err := os.Chmod(source, 0o644); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(dest, []byte("sentinel"), 0o600); err != nil {
		t.Fatal(err)
	}

	err := createTarHardlinkSecure(dest, root, source, &tar.Header{Mode: 0o600})
	if err == nil || !strings.Contains(err.Error(), "declared mode") {
		t.Fatalf("mode conflict error=%v", err)
	}
	got, readErr := os.ReadFile(dest)
	if readErr != nil {
		t.Fatal(readErr)
	}
	if string(got) != "sentinel" {
		t.Fatalf("destination changed after rejected metadata: %q", got)
	}
}

func TestCreateTarHardlinkSecureRejectsXattrConflictBeforeDestinationMutation(t *testing.T) {
	root := t.TempDir()
	source := filepath.Join(root, "source")
	dest := filepath.Join(root, "dest")
	if err := os.WriteFile(source, []byte("source"), 0o644); err != nil {
		t.Fatal(err)
	}
	if err := os.Chmod(source, 0o644); err != nil {
		t.Fatal(err)
	}
	const xattrName = "user.minicontainer.hardlink"
	if err := unix.Setxattr(source, xattrName, []byte("source-value"), 0); err != nil {
		if errors.Is(err, unix.ENOTSUP) || errors.Is(err, unix.EOPNOTSUPP) {
			t.Skipf("filesystem does not support user xattrs: %v", err)
		}
		t.Fatal(err)
	}
	if err := os.WriteFile(dest, []byte("sentinel"), 0o600); err != nil {
		t.Fatal(err)
	}

	hdr := &tar.Header{
		Mode: 0o644,
		Uid:  os.Getuid(),
		Gid:  os.Getgid(),
		PAXRecords: map[string]string{
			paxSchilyXattrPrefix + xattrName: "different-value",
		},
	}
	err := createTarHardlinkSecure(dest, root, source, hdr)
	if err == nil || !strings.Contains(err.Error(), "declared xattr") {
		t.Fatalf("xattr conflict error=%v", err)
	}
	got, readErr := os.ReadFile(dest)
	if readErr != nil {
		t.Fatal(readErr)
	}
	if string(got) != "sentinel" {
		t.Fatalf("destination changed after rejected xattr metadata: %q", got)
	}
}

func TestCreateTarHardlinkSecureAcceptsMatchingPinnedMetadata(t *testing.T) {
	root := t.TempDir()
	source := filepath.Join(root, "source")
	dest := filepath.Join(root, "dest")
	if err := os.WriteFile(source, []byte("source"), 0o640); err != nil {
		t.Fatal(err)
	}
	if err := os.Chmod(source, 0o640); err != nil {
		t.Fatal(err)
	}
	mtime := time.Unix(1_700_000_000, 123_000_000)
	if err := os.Chtimes(source, mtime, mtime); err != nil {
		t.Fatal(err)
	}
	const xattrName = "user.minicontainer.hardlink"
	if err := unix.Setxattr(source, xattrName, []byte("same-value"), 0); err != nil {
		if errors.Is(err, unix.ENOTSUP) || errors.Is(err, unix.EOPNOTSUPP) {
			t.Skipf("filesystem does not support user xattrs: %v", err)
		}
		t.Fatal(err)
	}
	info, err := os.Stat(source)
	if err != nil {
		t.Fatal(err)
	}
	hdr := &tar.Header{
		Mode:    0o640,
		Uid:     os.Getuid(),
		Gid:     os.Getgid(),
		ModTime: mtime,
		PAXRecords: map[string]string{
			paxSchilyXattrPrefix + xattrName: "same-value",
		},
	}
	if err := createTarHardlinkSecure(dest, root, source, hdr); err != nil {
		t.Fatalf("matching hardlink metadata: %v", err)
	}
	dstInfo, err := os.Stat(dest)
	if err != nil {
		t.Fatal(err)
	}
	if !os.SameFile(info, dstInfo) {
		t.Fatal("destination is not a hardlink to source")
	}
}

func TestVerifyPinnedHardlinkMetadataRejectsRootOwnershipConflict(t *testing.T) {
	st := unix.Stat_t{Mode: unix.S_IFREG | 0o644, Uid: 100, Gid: 200}
	hdr := &tar.Header{Mode: 0o644, Uid: 101, Gid: 200}
	if err := verifyPinnedHardlinkMetadata(st, hdr, 0); err == nil || !strings.Contains(err.Error(), "ownership") {
		t.Fatalf("ownership conflict error=%v", err)
	}
	if err := verifyPinnedHardlinkMetadata(st, hdr, 1000); err != nil {
		t.Fatalf("rootless ownership fallback should not reject: %v", err)
	}
}

func TestVerifyDeclaredXattrsPinnedFDFailsClosedForSymlink(t *testing.T) {
	err := verifyDeclaredXattrsPinnedFD(-1, unix.S_IFLNK|0o777, "link", map[string][]byte{"user.test": []byte("value")})
	if err == nil || !strings.Contains(err.Error(), "pinned symlink") {
		t.Fatalf("symlink xattr verification error=%v", err)
	}
}
