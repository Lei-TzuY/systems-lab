//go:build linux

package logs

import (
	"fmt"
	"os"
	"path/filepath"

	"golang.org/x/sys/unix"
)

func openContainerLogForAppend(path string) (*os.File, error) {
	return openContainerLog(path, unix.O_WRONLY|unix.O_CREAT|unix.O_APPEND, 0o600, true)
}

func openContainerLogForRead(path string) (*os.File, error) {
	return openContainerLog(path, unix.O_RDONLY, 0, false)
}

func openContainerLogForRotate(path string) (*os.File, error) {
	return openContainerLog(path, unix.O_RDWR, 0, false)
}

func openContainerLog(path string, flags int, mode uint32, createDir bool) (*os.File, error) {
	if isManagedLogPath(path) {
		return openManagedContainerLog(path, flags, mode, createDir)
	}

	dir := filepath.Dir(path)
	if createDir {
		if err := os.MkdirAll(dir, 0o700); err != nil {
			return nil, fmt.Errorf("create container log directory: %w", err)
		}
	}

	dfd, err := unix.Open(dir, unix.O_RDONLY|unix.O_DIRECTORY|unix.O_CLOEXEC|unix.O_NOFOLLOW, 0)
	if err != nil {
		return nil, fmt.Errorf("open container log directory: %w", err)
	}
	defer unix.Close(dfd)
	if err := unix.Fchmod(dfd, 0o700); err != nil {
		return nil, fmt.Errorf("secure container log directory: %w", err)
	}
	return openLogAt(dfd, filepath.Base(path), path, flags, mode)
}

func openManagedContainerLog(path string, flags int, mode uint32, createDir bool) (*os.File, error) {
	base := managedLogStateDir()
	if createDir {
		if err := unix.Mkdir(base, 0o700); err != nil && err != unix.EEXIST {
			return nil, fmt.Errorf("create log state directory: %w", err)
		}
	}

	baseFD, err := unix.Open(base, unix.O_RDONLY|unix.O_DIRECTORY|unix.O_CLOEXEC|unix.O_NOFOLLOW, 0)
	if err != nil {
		return nil, fmt.Errorf("open log state directory: %w", err)
	}
	defer unix.Close(baseFD)
	if err := unix.Fchmod(baseFD, 0o700); err != nil {
		return nil, fmt.Errorf("secure log state directory: %w", err)
	}

	if createDir {
		if err := unix.Mkdirat(baseFD, "containers", 0o700); err != nil && err != unix.EEXIST {
			return nil, fmt.Errorf("create container log directory: %w", err)
		}
	}
	logDirFD, err := unix.Openat(baseFD, "containers", unix.O_RDONLY|unix.O_DIRECTORY|unix.O_CLOEXEC|unix.O_NOFOLLOW, 0)
	if err != nil {
		return nil, fmt.Errorf("open container log directory: %w", err)
	}
	defer unix.Close(logDirFD)
	if err := unix.Fchmod(logDirFD, 0o700); err != nil {
		return nil, fmt.Errorf("secure container log directory: %w", err)
	}

	return openLogAt(logDirFD, filepath.Base(path), path, flags, mode)
}

func openLogAt(dirFD int, name, displayPath string, flags int, mode uint32) (*os.File, error) {
	fd, err := unix.Openat(dirFD, name, flags|unix.O_CLOEXEC|unix.O_NOFOLLOW|unix.O_NONBLOCK, mode)
	if err != nil {
		return nil, fmt.Errorf("open container log: %w", err)
	}

	var st unix.Stat_t
	if err := unix.Fstat(fd, &st); err != nil {
		unix.Close(fd)
		return nil, fmt.Errorf("stat container log: %w", err)
	}
	if st.Mode&unix.S_IFMT != unix.S_IFREG {
		unix.Close(fd)
		return nil, fmt.Errorf("container log is not a regular file")
	}
	if err := unix.Fchmod(fd, 0o600); err != nil {
		unix.Close(fd)
		return nil, fmt.Errorf("secure container log permissions: %w", err)
	}

	file := os.NewFile(uintptr(fd), displayPath)
	if file == nil {
		unix.Close(fd)
		return nil, fmt.Errorf("wrap container log fd")
	}
	return file, nil
}
