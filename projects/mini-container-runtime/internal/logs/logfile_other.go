//go:build !linux

package logs

import (
	"fmt"
	"os"
	"path/filepath"
)

func openContainerLogForAppend(path string) (*os.File, error) {
	return openContainerLogPortable(path, os.O_WRONLY|os.O_CREATE|os.O_APPEND, 0o600, true)
}

func openContainerLogForRead(path string) (*os.File, error) {
	return openContainerLogPortable(path, os.O_RDONLY, 0, false)
}

func openContainerLogForRotate(path string) (*os.File, error) {
	return openContainerLogPortable(path, os.O_RDWR, 0, false)
}

func openContainerLogPortable(path string, flags int, mode os.FileMode, createDir bool) (*os.File, error) {
	if isManagedLogPath(path) {
		if err := ensureManagedLogStoragePortable(createDir); err != nil {
			return nil, err
		}
	} else {
		dir := filepath.Dir(path)
		if createDir {
			if err := os.MkdirAll(dir, 0o700); err != nil {
				return nil, fmt.Errorf("create container log directory: %w", err)
			}
		}
		if info, err := os.Lstat(dir); err != nil {
			return nil, fmt.Errorf("inspect container log directory: %w", err)
		} else if info.Mode()&os.ModeSymlink != 0 || !info.IsDir() {
			return nil, fmt.Errorf("container log directory is not a real directory")
		}
		if err := os.Chmod(dir, 0o700); err != nil {
			return nil, fmt.Errorf("secure container log directory: %w", err)
		}
	}

	if info, err := os.Lstat(path); err == nil {
		if info.Mode()&os.ModeSymlink != 0 || !info.Mode().IsRegular() {
			return nil, fmt.Errorf("container log is not a regular file")
		}
	} else if !os.IsNotExist(err) {
		return nil, fmt.Errorf("inspect container log: %w", err)
	}

	f, err := os.OpenFile(path, flags, mode)
	if err != nil {
		return nil, fmt.Errorf("open container log: %w", err)
	}
	info, err := f.Stat()
	if err != nil {
		f.Close()
		return nil, fmt.Errorf("stat container log: %w", err)
	}
	if !info.Mode().IsRegular() {
		f.Close()
		return nil, fmt.Errorf("container log is not a regular file")
	}
	if err := f.Chmod(0o600); err != nil {
		f.Close()
		return nil, fmt.Errorf("secure container log permissions: %w", err)
	}
	return f, nil
}

func ensureManagedLogStoragePortable(create bool) error {
	base := managedLogStateDir()
	if create {
		if err := os.Mkdir(base, 0o700); err != nil && !os.IsExist(err) {
			return fmt.Errorf("create log state directory: %w", err)
		}
	}
	baseInfo, err := os.Lstat(base)
	if err != nil {
		return fmt.Errorf("inspect log state directory: %w", err)
	}
	if baseInfo.Mode()&os.ModeSymlink != 0 || !baseInfo.IsDir() {
		return fmt.Errorf("log state directory is not a real directory")
	}
	if err := os.Chmod(base, 0o700); err != nil {
		return fmt.Errorf("secure log state directory: %w", err)
	}

	dir := managedLogDir()
	if create {
		if err := os.Mkdir(dir, 0o700); err != nil && !os.IsExist(err) {
			return fmt.Errorf("create container log directory: %w", err)
		}
	}
	dirInfo, err := os.Lstat(dir)
	if err != nil {
		return fmt.Errorf("inspect container log directory: %w", err)
	}
	if dirInfo.Mode()&os.ModeSymlink != 0 || !dirInfo.IsDir() {
		return fmt.Errorf("container log directory is not a real directory")
	}
	if err := os.Chmod(dir, 0o700); err != nil {
		return fmt.Errorf("secure container log directory: %w", err)
	}
	return nil
}
