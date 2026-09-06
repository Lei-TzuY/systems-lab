//go:build linux

package main

import (
	"errors"
	"os"
	"syscall"
	"testing"
)

func TestSealInheritedFDsForPayloadInventoriesNonStdioDescriptors(t *testing.T) {
	readPipe, writePipe, err := os.Pipe()
	if err != nil {
		t.Fatal(err)
	}
	defer readPipe.Close()
	defer writePipe.Close()

	seen := map[int]bool{}
	if err := sealInheritedFDsForPayloadWith(func(fd int) error {
		if fd < 3 {
			t.Fatalf("stdio descriptor %d offered for sealing", fd)
		}
		seen[fd] = true
		return nil
	}); err != nil {
		t.Fatalf("inventory inherited fds: %v", err)
	}
	if !seen[int(readPipe.Fd())] || !seen[int(writePipe.Fd())] {
		t.Fatalf("pipe fds missing from inherited inventory: read=%d seen=%v write=%d seen=%v", readPipe.Fd(), seen[int(readPipe.Fd())], writePipe.Fd(), seen[int(writePipe.Fd())])
	}
}

func TestSealInheritedFDsForPayloadFailsClosedOnSealError(t *testing.T) {
	file, err := os.CreateTemp(t.TempDir(), "inherited-")
	if err != nil {
		t.Fatal(err)
	}
	defer file.Close()

	cause := errors.New("fcntl denied")
	calls := 0
	err = sealInheritedFDsForPayloadWith(func(fd int) error {
		calls++
		if fd == int(file.Fd()) {
			return cause
		}
		return nil
	})
	if !errors.Is(err, cause) {
		t.Fatalf("error=%v, want seal cause", err)
	}
	if calls == 0 {
		t.Fatal("sealer was not called")
	}
}

func TestSealInheritedFDsForPayloadToleratesVanishedDescriptor(t *testing.T) {
	file, err := os.CreateTemp(t.TempDir(), "vanished-")
	if err != nil {
		t.Fatal(err)
	}
	defer file.Close()

	if err := sealInheritedFDsForPayloadWith(func(fd int) error {
		if fd == int(file.Fd()) {
			return syscall.EBADF
		}
		return nil
	}); err != nil {
		t.Fatalf("vanished descriptor should already satisfy invariant: %v", err)
	}
}

func TestSetCloseOnExecSetsDescriptorFlag(t *testing.T) {
	file, err := os.CreateTemp(t.TempDir(), "cloexec-")
	if err != nil {
		t.Fatal(err)
	}
	defer file.Close()
	fd := file.Fd()

	_, _, errno := syscall.Syscall(syscall.SYS_FCNTL, fd, syscall.F_SETFD, 0)
	if errno != 0 {
		t.Fatalf("clear FD_CLOEXEC: %v", errno)
	}
	if err := setCloseOnExec(int(fd)); err != nil {
		t.Fatalf("set close-on-exec: %v", err)
	}
	flags, _, errno := syscall.Syscall(syscall.SYS_FCNTL, fd, syscall.F_GETFD, 0)
	if errno != 0 {
		t.Fatalf("get fd flags: %v", errno)
	}
	if flags&syscall.FD_CLOEXEC == 0 {
		t.Fatalf("fd %d is not close-on-exec", fd)
	}
}
