//go:build linux

package container

import (
	"fmt"
	"syscall"
)

type mountCall func(source, target, fstype string, flags uintptr, data string) error

func enforceReadOnlyRoot(readOnly, debug bool) error {
	return enforceReadOnlyRootWithMount(readOnly, debug, syscall.Mount)
}

func enforceReadOnlyRootWithMount(readOnly, debug bool, mount mountCall) error {
	if !readOnly {
		return nil
	}
	if mount == nil {
		return fmt.Errorf("read-only root remount function is nil")
	}
	if err := mount("", "/", "", syscall.MS_BIND|syscall.MS_REMOUNT|syscall.MS_RDONLY, ""); err != nil {
		return fmt.Errorf("remount root read-only: %w", err)
	}
	if debug {
		fmt.Println("[init] container rootfs remounted read-only")
	}
	return nil
}
