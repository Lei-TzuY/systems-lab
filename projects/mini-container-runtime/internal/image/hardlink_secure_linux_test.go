//go:build linux

package image

import (
	"errors"
	"os"
	"path/filepath"
	"testing"

	"golang.org/x/sys/unix"
)

func TestCreateHardlinkSecurePinsSourceAndDestinationParents(t *testing.T) {
	root := t.TempDir()
	sourceParent := filepath.Join(root, "src")
	destParent := filepath.Join(root, "dst")
	if err := os.Mkdir(sourceParent, 0755); err != nil {
		t.Fatal(err)
	}
	if err := os.Mkdir(destParent, 0755); err != nil {
		t.Fatal(err)
	}

	source := filepath.Join(sourceParent, "payload")
	if err := os.WriteFile(source, []byte("owned-generation"), 0644); err != nil {
		t.Fatal(err)
	}

	outsideSource := t.TempDir()
	outsideDest := t.TempDir()
	if err := os.WriteFile(filepath.Join(outsideSource, "payload"), []byte("foreign-generation"), 0644); err != nil {
		t.Fatal(err)
	}

	pinnedSourceParent := filepath.Join(root, "src-pinned")
	pinnedDestParent := filepath.Join(root, "dst-pinned")
	target := filepath.Join(destParent, "copy")

	err := createHardlinkSecureWithHook(target, root, source, func() {
		if err := os.Rename(sourceParent, pinnedSourceParent); err != nil {
			t.Fatal(err)
		}
		if err := os.Symlink(outsideSource, sourceParent); err != nil {
			t.Fatal(err)
		}
		if err := os.Rename(destParent, pinnedDestParent); err != nil {
			t.Fatal(err)
		}
		if err := os.Symlink(outsideDest, destParent); err != nil {
			t.Fatal(err)
		}
	})
	if err != nil {
		t.Fatalf("secure hardlink after parent replacement: %v", err)
	}

	got, err := os.ReadFile(filepath.Join(pinnedDestParent, "copy"))
	if err != nil {
		t.Fatal(err)
	}
	if string(got) != "owned-generation" {
		t.Fatalf("hardlink content = %q, want pinned source content", got)
	}
	if _, err := os.Lstat(filepath.Join(outsideDest, "copy")); !os.IsNotExist(err) {
		t.Fatalf("outside destination was modified: err=%v", err)
	}

	sourceInfo, err := os.Stat(filepath.Join(pinnedSourceParent, "payload"))
	if err != nil {
		t.Fatal(err)
	}
	linkInfo, err := os.Stat(filepath.Join(pinnedDestParent, "copy"))
	if err != nil {
		t.Fatal(err)
	}
	if !os.SameFile(sourceInfo, linkInfo) {
		t.Fatal("destination is not a hardlink to the pinned source inode")
	}
}

func TestCreateHardlinkSecurePinsSourceLeafAcrossReplacement(t *testing.T) {
	root := t.TempDir()
	source := filepath.Join(root, "source")
	original := filepath.Join(root, "source-original")
	target := filepath.Join(root, "copy")
	if err := os.WriteFile(source, []byte("verified-inode"), 0644); err != nil {
		t.Fatal(err)
	}

	err := createHardlinkSecureWithHook(target, root, source, func() {
		if err := os.Rename(source, original); err != nil {
			t.Fatal(err)
		}
		if err := os.WriteFile(source, []byte("replacement-inode"), 0644); err != nil {
			t.Fatal(err)
		}
	})
	if err != nil {
		t.Fatalf("secure hardlink after source leaf replacement: %v", err)
	}

	got, err := os.ReadFile(target)
	if err != nil {
		t.Fatal(err)
	}
	if string(got) != "verified-inode" {
		t.Fatalf("hardlink followed replacement source: got %q", got)
	}
	originalInfo, err := os.Stat(original)
	if err != nil {
		t.Fatal(err)
	}
	linkInfo, err := os.Stat(target)
	if err != nil {
		t.Fatal(err)
	}
	if !os.SameFile(originalInfo, linkInfo) {
		t.Fatal("destination does not link the source inode pinned before replacement")
	}
	replacementInfo, err := os.Stat(source)
	if err != nil {
		t.Fatal(err)
	}
	if os.SameFile(replacementInfo, linkInfo) {
		t.Fatal("destination unexpectedly links the replacement source inode")
	}
}

func TestLinkPinnedHardlinkSourceFailsClosedForSymlinkWhenEmptyPathUnavailable(t *testing.T) {
	cause := unix.EPERM
	calls := 0
	err := linkPinnedHardlinkSource(41, unix.S_IFLNK|0777, 52, "copy", func(olddirfd int, oldpath string, newdirfd int, newpath string, flags int) error {
		calls++
		if calls != 1 {
			t.Fatalf("symlink source reached procfs fallback call %d", calls)
		}
		if olddirfd != 41 || oldpath != "" || newdirfd != 52 || newpath != "copy" || flags != unix.AT_EMPTY_PATH {
			t.Fatalf("unexpected fd-native link args: old=%d/%q new=%d/%q flags=%#x", olddirfd, oldpath, newdirfd, newpath, flags)
		}
		return cause
	})
	if !errors.Is(err, cause) {
		t.Fatalf("symlink fallback error = %v, want underlying %v", err, cause)
	}
	if calls != 1 {
		t.Fatalf("link calls=%d, want only AT_EMPTY_PATH attempt", calls)
	}
}

func TestLinkPinnedHardlinkSourceUsesProcFallbackForRegularFile(t *testing.T) {
	calls := 0
	err := linkPinnedHardlinkSource(61, unix.S_IFREG|0644, 72, "copy", func(olddirfd int, oldpath string, newdirfd int, newpath string, flags int) error {
		calls++
		switch calls {
		case 1:
			if olddirfd != 61 || oldpath != "" || newdirfd != 72 || newpath != "copy" || flags != unix.AT_EMPTY_PATH {
				t.Fatalf("unexpected fd-native link args: old=%d/%q new=%d/%q flags=%#x", olddirfd, oldpath, newdirfd, newpath, flags)
			}
			return unix.EPERM
		case 2:
			if olddirfd != unix.AT_FDCWD || oldpath != "/proc/self/fd/61" || newdirfd != 72 || newpath != "copy" || flags != unix.AT_SYMLINK_FOLLOW {
				t.Fatalf("unexpected proc fallback args: old=%d/%q new=%d/%q flags=%#x", olddirfd, oldpath, newdirfd, newpath, flags)
			}
			return nil
		default:
			t.Fatalf("unexpected link call %d", calls)
			return nil
		}
	})
	if err != nil {
		t.Fatalf("regular-file proc fallback: %v", err)
	}
	if calls != 2 {
		t.Fatalf("link calls=%d, want AT_EMPTY_PATH plus proc fallback", calls)
	}
}

func TestCreateHardlinkSecureRefusesDirectoryDestination(t *testing.T) {
	root := t.TempDir()
	if err := os.WriteFile(filepath.Join(root, "source"), []byte("payload"), 0644); err != nil {
		t.Fatal(err)
	}
	target := filepath.Join(root, "existing-dir")
	if err := os.Mkdir(target, 0755); err != nil {
		t.Fatal(err)
	}
	marker := filepath.Join(target, "keep")
	if err := os.WriteFile(marker, []byte("keep"), 0644); err != nil {
		t.Fatal(err)
	}

	if err := createHardlinkSecure(target, root, filepath.Join(root, "source")); err == nil {
		t.Fatal("directory hardlink destination was accepted")
	}
	if got, err := os.ReadFile(marker); err != nil || string(got) != "keep" {
		t.Fatalf("directory destination was destructively modified: content=%q err=%v", got, err)
	}
}

func TestCreateHardlinkSecureRejectsSymlinkParent(t *testing.T) {
	root := t.TempDir()
	outside := t.TempDir()
	if err := os.WriteFile(filepath.Join(root, "source"), []byte("payload"), 0644); err != nil {
		t.Fatal(err)
	}
	if err := os.Symlink(outside, filepath.Join(root, "dst")); err != nil {
		t.Fatal(err)
	}

	err := createHardlinkSecure(filepath.Join(root, "dst", "copy"), root, filepath.Join(root, "source"))
	if err == nil {
		t.Fatal("symlink destination parent was accepted")
	}
	if _, err := os.Lstat(filepath.Join(outside, "copy")); !os.IsNotExist(err) {
		t.Fatalf("outside directory was modified: err=%v", err)
	}
}
