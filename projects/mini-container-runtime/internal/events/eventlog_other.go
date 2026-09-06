//go:build !linux

package events

import (
	"fmt"
	"os"
	"path/filepath"
)

func openEventLogForAppend(path string) (*os.File, error) {
	return openEventLogPortable(path, os.O_WRONLY|os.O_CREATE|os.O_APPEND, 0o600)
}

func openEventLogForRead(path string) (*os.File, error) {
	return openEventLogPortable(path, os.O_RDONLY, 0)
}

func openEventLogPortable(path string, flags int, mode os.FileMode) (*os.File, error) {
	if isManagedEventLogPath(path) {
		if err := ensureManagedEventStatePortable(flags&os.O_CREATE != 0); err != nil {
			return nil, err
		}
	} else {
		dir := filepath.Dir(path)
		if flags&os.O_CREATE != 0 {
			if err := os.MkdirAll(dir, 0o700); err != nil {
				return nil, fmt.Errorf("create event log directory: %w", err)
			}
		}
		info, err := os.Lstat(dir)
		if err != nil {
			return nil, fmt.Errorf("inspect event log directory: %w", err)
		}
		if info.Mode()&os.ModeSymlink != 0 || !info.IsDir() {
			return nil, fmt.Errorf("event log directory is not a real directory")
		}
		if err := os.Chmod(dir, 0o700); err != nil {
			return nil, fmt.Errorf("secure event log directory: %w", err)
		}
	}

	if info, err := os.Lstat(path); err == nil {
		if info.Mode()&os.ModeSymlink != 0 || !info.Mode().IsRegular() {
			return nil, fmt.Errorf("event log is not a regular file")
		}
	} else if !os.IsNotExist(err) {
		return nil, fmt.Errorf("inspect event log: %w", err)
	}

	f, err := os.OpenFile(path, flags, mode)
	if err != nil {
		return nil, fmt.Errorf("open event log: %w", err)
	}
	info, err := f.Stat()
	if err != nil {
		f.Close()
		return nil, fmt.Errorf("stat event log: %w", err)
	}
	if !info.Mode().IsRegular() {
		f.Close()
		return nil, fmt.Errorf("event log is not a regular file")
	}
	if flags&(os.O_WRONLY|os.O_RDWR) != 0 {
		if err := f.Chmod(0o600); err != nil {
			f.Close()
			return nil, fmt.Errorf("secure event log permissions: %w", err)
		}
	}
	return f, nil
}

func ensureManagedEventStatePortable(create bool) error {
	base := eventStateDir()
	if create {
		if err := os.Mkdir(base, 0o700); err != nil && !os.IsExist(err) {
			return fmt.Errorf("create event state directory: %w", err)
		}
	}
	info, err := os.Lstat(base)
	if err != nil {
		return fmt.Errorf("inspect event state directory: %w", err)
	}
	if info.Mode()&os.ModeSymlink != 0 || !info.IsDir() {
		return fmt.Errorf("event state directory is not a real directory")
	}
	if err := os.Chmod(base, 0o700); err != nil {
		return fmt.Errorf("secure event state directory: %w", err)
	}
	return nil
}
