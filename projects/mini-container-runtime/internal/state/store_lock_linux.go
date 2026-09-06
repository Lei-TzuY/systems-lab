//go:build linux

package state

import (
	"fmt"
	"os"

	"golang.org/x/sys/unix"
)

func openStateLock(path string) (*os.File, error) {
	fd, err := unix.Open(path, unix.O_CREAT|unix.O_RDWR|unix.O_CLOEXEC|unix.O_NOFOLLOW, 0o600)
	if err != nil {
		return nil, fmt.Errorf("open state lock: %w", err)
	}
	file := os.NewFile(uintptr(fd), path)
	if file == nil {
		_ = unix.Close(fd)
		return nil, fmt.Errorf("wrap state lock fd")
	}
	info, err := file.Stat()
	if err != nil {
		_ = file.Close()
		return nil, fmt.Errorf("stat state lock: %w", err)
	}
	if !info.Mode().IsRegular() {
		_ = file.Close()
		return nil, fmt.Errorf("state lock %q is not a regular file", path)
	}
	if err := file.Chmod(0o600); err != nil {
		_ = file.Close()
		return nil, fmt.Errorf("secure state lock permissions: %w", err)
	}
	return file, nil
}

func lockStateFile(file *os.File) error {
	if file == nil {
		return ErrStoreClosed
	}
	if err := unix.Flock(int(file.Fd()), unix.LOCK_EX); err != nil {
		return fmt.Errorf("lock state store: %w", err)
	}
	return nil
}

func unlockStateFile(file *os.File) error {
	if file == nil {
		return nil
	}
	if err := unix.Flock(int(file.Fd()), unix.LOCK_UN); err != nil {
		return fmt.Errorf("unlock state store: %w", err)
	}
	return nil
}
