//go:build linux

package main

import (
	"os"
	"strconv"
	"syscall"
	"testing"
)

func TestSealPinnedRootFSFDForPayloadSetsCloseOnExec(t *testing.T) {
	rootfs, err := os.Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	defer rootfs.Close()

	fd := rootfs.Fd()
	if err := sealPinnedRootFSFDForPayload(strconv.FormatUint(uint64(fd), 10)); err != nil {
		t.Fatalf("seal rootfs fd: %v", err)
	}

	flags, _, errno := syscall.Syscall(syscall.SYS_FCNTL, fd, syscall.F_GETFD, 0)
	if errno != 0 {
		t.Fatalf("fcntl F_GETFD: %v", errno)
	}
	if flags&syscall.FD_CLOEXEC == 0 {
		t.Fatalf("rootfs fd %d is not close-on-exec", fd)
	}
	if _, err := rootfs.Stat(); err != nil {
		t.Fatalf("rootfs fd was closed before payload exec: %v", err)
	}
}

func TestSealPinnedRootFSFDForPayloadRejectsMalformedFD(t *testing.T) {
	for _, raw := range []string{"", "not-a-fd", "-1", "0", "2"} {
		if err := sealPinnedRootFSFDForPayload(raw); err == nil {
			t.Fatalf("expected invalid fd %q to be rejected", raw)
		}
	}
}

func TestSealPinnedRootFSFDForPayloadRejectsNonDirectory(t *testing.T) {
	file, err := os.CreateTemp(t.TempDir(), "not-rootfs-")
	if err != nil {
		t.Fatal(err)
	}
	defer file.Close()

	if err := sealPinnedRootFSFDForPayload(strconv.FormatUint(uint64(file.Fd()), 10)); err == nil {
		t.Fatal("expected non-directory inherited fd to be rejected")
	}
}
