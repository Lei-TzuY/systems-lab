//go:build linux

package image

import (
	"errors"
	"os"
	"path/filepath"
	"testing"

	"golang.org/x/sys/unix"
)

func openHardlinkPublishTestFDs(t *testing.T, root, source string) (int, int) {
	t.Helper()
	parentFD, err := unix.Open(root, unix.O_RDONLY|unix.O_DIRECTORY|unix.O_CLOEXEC, 0)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = unix.Close(parentFD) })
	sourceFD, err := unix.Open(source, unix.O_PATH|unix.O_CLOEXEC|unix.O_NOFOLLOW, 0)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = unix.Close(sourceFD) })
	return parentFD, sourceFD
}

func TestPublishPinnedHardlinkPreservesDestinationWhenLinkFails(t *testing.T) {
	root := t.TempDir()
	source := filepath.Join(root, "source")
	dest := filepath.Join(root, "dest")
	if err := os.WriteFile(source, []byte("new"), 0644); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(dest, []byte("keep-old"), 0600); err != nil {
		t.Fatal(err)
	}
	parentFD, sourceFD := openHardlinkPublishTestFDs(t, root, source)

	cause := unix.EIO
	renameCalls := 0
	err := publishPinnedHardlinkSource(
		sourceFD,
		unix.S_IFREG|0644,
		parentFD,
		"dest",
		func(int, string, int, string, int) error { return cause },
		func(int, string, int, string) error { renameCalls++; return nil },
		func() (string, error) { return ".hardlink-stage", nil },
	)
	if !errors.Is(err, cause) {
		t.Fatalf("publish error=%v, want underlying %v", err, cause)
	}
	if renameCalls != 0 {
		t.Fatalf("rename called %d times after staging link failure", renameCalls)
	}
	got, err := os.ReadFile(dest)
	if err != nil || string(got) != "keep-old" {
		t.Fatalf("existing destination changed after link failure: content=%q err=%v", got, err)
	}
	if _, err := os.Lstat(filepath.Join(root, ".hardlink-stage")); !os.IsNotExist(err) {
		t.Fatalf("unexpected staging debris after link failure: %v", err)
	}
}

func TestPublishPinnedHardlinkCleansStageAndPreservesDestinationWhenRenameFails(t *testing.T) {
	root := t.TempDir()
	source := filepath.Join(root, "source")
	dest := filepath.Join(root, "dest")
	if err := os.WriteFile(source, []byte("new"), 0644); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(dest, []byte("keep-old"), 0600); err != nil {
		t.Fatal(err)
	}
	parentFD, sourceFD := openHardlinkPublishTestFDs(t, root, source)

	cause := unix.EIO
	err := publishPinnedHardlinkSource(
		sourceFD,
		unix.S_IFREG|0644,
		parentFD,
		"dest",
		func(_ int, _ string, newdirfd int, newpath string, _ int) error {
			return unix.Linkat(unix.AT_FDCWD, source, newdirfd, newpath, 0)
		},
		func(int, string, int, string) error { return cause },
		func() (string, error) { return ".hardlink-stage", nil },
	)
	if !errors.Is(err, cause) {
		t.Fatalf("publish error=%v, want underlying %v", err, cause)
	}
	got, err := os.ReadFile(dest)
	if err != nil || string(got) != "keep-old" {
		t.Fatalf("existing destination changed after rename failure: content=%q err=%v", got, err)
	}
	if _, err := os.Lstat(filepath.Join(root, ".hardlink-stage")); !os.IsNotExist(err) {
		t.Fatalf("staging hardlink leaked after rename failure: %v", err)
	}
}

func TestCreateHardlinkSecureAtomicallyReplacesExistingLeaf(t *testing.T) {
	root := t.TempDir()
	source := filepath.Join(root, "source")
	dest := filepath.Join(root, "dest")
	if err := os.WriteFile(source, []byte("new"), 0644); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(dest, []byte("old"), 0600); err != nil {
		t.Fatal(err)
	}

	if err := createHardlinkSecure(dest, root, source); err != nil {
		t.Fatalf("replace existing hardlink destination: %v", err)
	}
	sourceInfo, err := os.Stat(source)
	if err != nil {
		t.Fatal(err)
	}
	destInfo, err := os.Stat(dest)
	if err != nil {
		t.Fatal(err)
	}
	if !os.SameFile(sourceInfo, destInfo) {
		t.Fatal("published destination is not the source hardlink")
	}
	got, err := os.ReadFile(dest)
	if err != nil || string(got) != "new" {
		t.Fatalf("published destination content=%q err=%v", got, err)
	}
}
