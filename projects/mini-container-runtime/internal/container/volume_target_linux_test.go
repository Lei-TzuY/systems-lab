//go:build linux

package container

import (
	"errors"
	"os"
	"path/filepath"
	"testing"

	"golang.org/x/sys/unix"
)

func TestNormalizeVolumeContainerPathRejectsTraversal(t *testing.T) {
	for _, input := range []string{
		"",
		"relative/path",
		"/../host",
		"/safe/../../host",
		"/safe/..",
		"/safe/../host",
		"/safe\x00/child",
	} {
		if _, err := normalizeVolumeContainerPath(input); err == nil {
			t.Fatalf("expected %q to be rejected", input)
		}
	}
}

func TestNormalizeVolumeContainerPathCanonicalizesSafePath(t *testing.T) {
	for input, want := range map[string]string{
		"/":                  ".",
		"/var//lib/./data/": "var/lib/data",
		"/simple":            "simple",
	} {
		got, err := normalizeVolumeContainerPath(input)
		if err != nil {
			t.Fatalf("normalize %q: %v", input, err)
		}
		if got != want {
			t.Fatalf("normalize %q = %q, want %q", input, got, want)
		}
	}
}

func TestOpenVolumeTargetCreatesDirectoryBeneathRoot(t *testing.T) {
	root := t.TempDir()
	fd, err := openVolumeTarget(root, "/var/lib/app/data")
	if err != nil {
		t.Fatalf("openVolumeTarget: %v", err)
	}
	defer unix.Close(fd)

	wantInfo, err := os.Stat(filepath.Join(root, "var", "lib", "app", "data"))
	if err != nil {
		t.Fatalf("stat created target: %v", err)
	}
	fdInfo, err := os.Stat(volumeTargetFDPath(fd))
	if err != nil {
		t.Fatalf("stat target fd: %v", err)
	}
	if !os.SameFile(wantInfo, fdInfo) {
		t.Fatal("target fd does not refer to the directory created beneath rootfs")
	}
}

func TestOpenat2VolumeTargetPreservesSafeInRootSymlink(t *testing.T) {
	root := t.TempDir()
	if err := os.Mkdir(filepath.Join(root, "real"), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.Symlink("/real", filepath.Join(root, "link")); err != nil {
		t.Fatal(err)
	}

	rootFD, err := unix.Open(root, unix.O_PATH|unix.O_DIRECTORY|unix.O_CLOEXEC, 0)
	if err != nil {
		t.Fatal(err)
	}
	defer unix.Close(rootFD)

	fd, err := openOrCreateVolumeTargetOpenat2(rootFD, "link/child")
	if errors.Is(err, unix.ENOSYS) {
		t.Skip("kernel does not support openat2")
	}
	if err != nil {
		t.Fatalf("openat2 safe symlink target: %v", err)
	}
	defer unix.Close(fd)

	wantInfo, err := os.Stat(filepath.Join(root, "real", "child"))
	if err != nil {
		t.Fatalf("safe symlink target was not created in rootfs: %v", err)
	}
	fdInfo, err := os.Stat(volumeTargetFDPath(fd))
	if err != nil {
		t.Fatal(err)
	}
	if !os.SameFile(wantInfo, fdInfo) {
		t.Fatal("safe in-root symlink resolved to the wrong directory")
	}
}

func TestOpenVolumeTargetCannotEscapeThroughAbsoluteSymlink(t *testing.T) {
	root := t.TempDir()
	outside := t.TempDir()
	if err := os.Symlink(outside, filepath.Join(root, "escape")); err != nil {
		t.Fatal(err)
	}

	fd, err := openVolumeTarget(root, "/escape/owned")
	if fd >= 0 {
		_ = unix.Close(fd)
	}
	if err == nil {
		t.Fatal("absolute symlink outside rootfs unexpectedly resolved")
	}
	if _, statErr := os.Stat(filepath.Join(outside, "owned")); !errors.Is(statErr, os.ErrNotExist) {
		t.Fatalf("outside directory was modified through mount target escape: %v", statErr)
	}
}

func TestVolumeTargetFDRemainsStableAfterPathSwap(t *testing.T) {
	root := t.TempDir()
	outside := t.TempDir()
	mountPath := filepath.Join(root, "mnt")
	if err := os.Mkdir(mountPath, 0o755); err != nil {
		t.Fatal(err)
	}

	fd, err := openVolumeTarget(root, "/mnt")
	if err != nil {
		t.Fatal(err)
	}
	defer unix.Close(fd)

	oldPath := filepath.Join(root, "mnt-old")
	if err := os.Rename(mountPath, oldPath); err != nil {
		t.Fatal(err)
	}
	if err := os.Symlink(outside, mountPath); err != nil {
		t.Fatal(err)
	}

	fdInfo, err := os.Stat(volumeTargetFDPath(fd))
	if err != nil {
		t.Fatal(err)
	}
	oldInfo, err := os.Stat(oldPath)
	if err != nil {
		t.Fatal(err)
	}
	outsideInfo, err := os.Stat(outside)
	if err != nil {
		t.Fatal(err)
	}
	if !os.SameFile(fdInfo, oldInfo) {
		t.Fatal("target fd changed identity after pathname swap")
	}
	if os.SameFile(fdInfo, outsideInfo) {
		t.Fatal("target fd followed replacement symlink outside rootfs")
	}
}

func TestNoSymlinkFallbackRejectsSymlinkComponents(t *testing.T) {
	root := t.TempDir()
	if err := os.Mkdir(filepath.Join(root, "real"), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.Symlink("real", filepath.Join(root, "link")); err != nil {
		t.Fatal(err)
	}

	rootFD, err := unix.Open(root, unix.O_PATH|unix.O_DIRECTORY|unix.O_CLOEXEC, 0)
	if err != nil {
		t.Fatal(err)
	}
	defer unix.Close(rootFD)

	fd, err := openOrCreateVolumeTargetNoSymlink(rootFD, "link/child")
	if fd >= 0 {
		_ = unix.Close(fd)
	}
	if err == nil {
		t.Fatal("old-kernel fallback followed a symlink component")
	}
}

func TestNoSymlinkFallbackCreatesOrdinaryDirectories(t *testing.T) {
	root := t.TempDir()
	rootFD, err := unix.Open(root, unix.O_PATH|unix.O_DIRECTORY|unix.O_CLOEXEC, 0)
	if err != nil {
		t.Fatal(err)
	}
	defer unix.Close(rootFD)

	fd, err := openOrCreateVolumeTargetNoSymlink(rootFD, "a/b/c")
	if err != nil {
		t.Fatalf("fallback ordinary path: %v", err)
	}
	defer unix.Close(fd)

	want, err := os.Stat(filepath.Join(root, "a", "b", "c"))
	if err != nil {
		t.Fatal(err)
	}
	got, err := os.Stat(volumeTargetFDPath(fd))
	if err != nil {
		t.Fatal(err)
	}
	if !os.SameFile(want, got) {
		t.Fatal("fallback target fd does not refer beneath rootfs")
	}
}
