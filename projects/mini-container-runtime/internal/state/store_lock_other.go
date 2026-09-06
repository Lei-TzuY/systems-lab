//go:build !linux

package state

import (
	"fmt"
	"os"
)

func openStateLock(path string) (*os.File, error) {
	info, err := os.Lstat(path)
	if err == nil && info.Mode()&os.ModeSymlink != 0 {
		return nil, fmt.Errorf("state lock %q must not be a symlink", path)
	}
	if err != nil && !os.IsNotExist(err) {
		return nil, fmt.Errorf("inspect state lock: %w", err)
	}
	file, err := os.OpenFile(path, os.O_CREATE|os.O_RDWR, 0o600)
	if err != nil {
		return nil, fmt.Errorf("open state lock: %w", err)
	}
	openedInfo, err := file.Stat()
	if err != nil {
		_ = file.Close()
		return nil, fmt.Errorf("stat state lock: %w", err)
	}
	if !openedInfo.Mode().IsRegular() {
		_ = file.Close()
		return nil, fmt.Errorf("state lock %q is not a regular file", path)
	}
	if err := file.Chmod(0o600); err != nil {
		_ = file.Close()
		return nil, fmt.Errorf("secure state lock permissions: %w", err)
	}
	return file, nil
}

// Non-Linux builds retain the existing process-local mutex semantics. The
// container runtime itself is Linux-only; these stubs keep utility packages and
// tests portable without pretending to provide a cross-process guarantee.
func lockStateFile(file *os.File) error {
	if file == nil {
		return ErrStoreClosed
	}
	return nil
}
func unlockStateFile(file *os.File) error { return nil }
