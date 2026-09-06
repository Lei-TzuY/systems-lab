//go:build linux

package container

import (
	"errors"
	"testing"

	"golang.org/x/sys/unix"
)

func openDistinctRootAndProcMounts(t *testing.T) int {
	t.Helper()
	rootFD, err := unix.Open("/", unix.O_PATH|unix.O_DIRECTORY|unix.O_CLOEXEC, 0)
	if err != nil {
		t.Fatal(err)
	}
	procFD, err := unix.Open("/proc", unix.O_PATH|unix.O_DIRECTORY|unix.O_CLOEXEC, 0)
	if err != nil {
		_ = unix.Close(rootFD)
		t.Skipf("/proc unavailable: %v", err)
	}
	rootMount, err := fdMountID(rootFD)
	if err != nil {
		_ = unix.Close(procFD)
		_ = unix.Close(rootFD)
		t.Fatal(err)
	}
	procMount, err := fdMountID(procFD)
	_ = unix.Close(procFD)
	if err != nil {
		_ = unix.Close(rootFD)
		t.Fatal(err)
	}
	if rootMount == procMount {
		_ = unix.Close(rootFD)
		t.Skip("/proc is not a separate mount in this environment")
	}
	return rootFD
}

func TestOpenat2AllowsExistingDirectoryOnSubmount(t *testing.T) {
	rootFD := openDistinctRootAndProcMounts(t)
	defer unix.Close(rootFD)

	fd, err := openVolumeDirInRoot(rootFD, "proc")
	if errors.Is(err, unix.ENOSYS) {
		t.Skip("kernel does not support openat2")
	}
	if err != nil {
		t.Fatalf("existing /proc directory should remain a valid target: %v", err)
	}
	_ = unix.Close(fd)
}

func TestOpenat2RejectsDirectoryCreationAfterCrossingMount(t *testing.T) {
	rootFD := openDistinctRootAndProcMounts(t)
	defer unix.Close(rootFD)

	fd, err := openOrCreateVolumeTargetOpenat2(rootFD, "proc/minicontainer-volume-target-must-not-be-created")
	if fd >= 0 {
		_ = unix.Close(fd)
	}
	if errors.Is(err, unix.ENOSYS) {
		t.Skip("kernel does not support openat2")
	}
	if !errors.Is(err, errVolumeTargetCrossMount) {
		t.Fatalf("cross-mount creation error=%v, want boundary error", err)
	}
}

func TestNoSymlinkFallbackAllowsExistingDirectoryOnSubmount(t *testing.T) {
	rootFD := openDistinctRootAndProcMounts(t)
	defer unix.Close(rootFD)

	fd, err := openOrCreateVolumeTargetNoSymlink(rootFD, "proc")
	if err != nil {
		t.Fatalf("fallback should allow existing /proc target: %v", err)
	}
	_ = unix.Close(fd)
}

func TestNoSymlinkFallbackRejectsCreationAfterCrossingMount(t *testing.T) {
	rootFD := openDistinctRootAndProcMounts(t)
	defer unix.Close(rootFD)

	fd, err := openOrCreateVolumeTargetNoSymlink(rootFD, "proc/minicontainer-volume-target-must-not-be-created")
	if fd >= 0 {
		_ = unix.Close(fd)
	}
	if !errors.Is(err, errVolumeTargetCrossMount) {
		t.Fatalf("fallback cross-mount creation error=%v, want boundary error", err)
	}
}

func TestFDMountIDIsStableWithinSameMount(t *testing.T) {
	root := t.TempDir()
	rootFD, err := unix.Open(root, unix.O_PATH|unix.O_DIRECTORY|unix.O_CLOEXEC, 0)
	if err != nil {
		t.Fatal(err)
	}
	defer unix.Close(rootFD)
	if err := unix.Mkdirat(rootFD, "child", 0o755); err != nil {
		t.Fatal(err)
	}
	childFD, err := unix.Openat(rootFD, "child", unix.O_PATH|unix.O_DIRECTORY|unix.O_CLOEXEC, 0)
	if err != nil {
		t.Fatal(err)
	}
	defer unix.Close(childFD)

	rootMount, err := fdMountID(rootFD)
	if err != nil {
		t.Fatal(err)
	}
	childMount, err := fdMountID(childFD)
	if err != nil {
		t.Fatal(err)
	}
	if rootMount != childMount {
		t.Fatalf("same-mount directory IDs differ: root=%d child=%d", rootMount, childMount)
	}
}
