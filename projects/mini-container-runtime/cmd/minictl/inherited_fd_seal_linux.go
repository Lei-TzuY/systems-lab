//go:build linux

package main

import (
	"errors"
	"fmt"
	"os"
	"strconv"
	"syscall"
)

// sealInheritedFDsForPayload marks every inherited non-stdio descriptor
// close-on-exec before runtime setup begins. Bootstrap code may continue using
// those descriptors, but the final payload exec receives no ambient runtime
// capability unless a future feature explicitly opts one back in.
func sealInheritedFDsForPayload() error {
	return sealInheritedFDsForPayloadWith(setCloseOnExec)
}

func sealInheritedFDsForPayloadWith(seal func(int) error) error {
	if seal == nil {
		return fmt.Errorf("inherited fd sealer is nil")
	}
	dir, err := os.Open("/proc/self/fd")
	if err != nil {
		return fmt.Errorf("open inherited fd inventory: %w", err)
	}
	dirFD := int(dir.Fd())
	names, readErr := dir.Readdirnames(-1)
	closeErr := dir.Close()
	if readErr != nil {
		return fmt.Errorf("read inherited fd inventory: %w", readErr)
	}
	if closeErr != nil {
		return fmt.Errorf("close inherited fd inventory: %w", closeErr)
	}

	for _, name := range names {
		fd, err := strconv.Atoi(name)
		if err != nil || fd < 3 || fd == dirFD {
			continue
		}
		if err := seal(fd); err != nil {
			// A descriptor may disappear between procfs enumeration and fcntl.
			// That already satisfies the no-inheritance invariant.
			if errors.Is(err, syscall.EBADF) {
				continue
			}
			return fmt.Errorf("seal inherited fd %d: %w", fd, err)
		}
	}
	return nil
}

func setCloseOnExec(fd int) error {
	flags, _, errno := syscall.Syscall(syscall.SYS_FCNTL, uintptr(fd), syscall.F_GETFD, 0)
	if errno != 0 {
		return errno
	}
	if flags&syscall.FD_CLOEXEC != 0 {
		return nil
	}
	_, _, errno = syscall.Syscall(syscall.SYS_FCNTL, uintptr(fd), syscall.F_SETFD, flags|syscall.FD_CLOEXEC)
	if errno != 0 {
		return errno
	}
	return nil
}
