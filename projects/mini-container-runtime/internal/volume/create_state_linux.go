//go:build linux

package volume

import (
	"errors"
	"fmt"
	"path/filepath"
	"strings"

	"golang.org/x/sys/unix"
)

// openOrCreateManagedStateDir walks every existing parent component from /
// without following symlinks and permits creation only for the final state-root
// component. This keeps even the first state mutation anchored to a pinned
// parent descriptor instead of pathname-based MkdirAll resolution.
func openOrCreateManagedStateDir(path string) (int, error) {
	clean := filepath.Clean(path)
	if !filepath.IsAbs(clean) || clean == string(filepath.Separator) {
		return -1, fmt.Errorf("managed state path %q must be an absolute non-root directory", path)
	}
	parts := strings.Split(strings.TrimPrefix(clean, string(filepath.Separator)), string(filepath.Separator))
	if len(parts) == 0 {
		return -1, fmt.Errorf("managed state path %q has no components", path)
	}

	fd, err := unix.Open("/", unix.O_RDONLY|unix.O_DIRECTORY|unix.O_CLOEXEC, 0)
	if err != nil {
		return -1, fmt.Errorf("open filesystem root: %w", err)
	}
	for i, part := range parts {
		if part == "" || part == "." || part == ".." {
			_ = unix.Close(fd)
			return -1, fmt.Errorf("invalid managed state path component %q", part)
		}
		last := i == len(parts)-1
		if last {
			if err := unix.Mkdirat(fd, part, 0o700); err != nil && !errors.Is(err, unix.EEXIST) {
				_ = unix.Close(fd)
				return -1, fmt.Errorf("create managed state directory: %w", err)
			}
		}
		next, err := unix.Openat(fd, part, unix.O_RDONLY|unix.O_DIRECTORY|unix.O_CLOEXEC|unix.O_NOFOLLOW, 0)
		_ = unix.Close(fd)
		if err != nil {
			if last {
				return -1, pinnedDirOpenError("state directory", err)
			}
			return -1, fmt.Errorf("pin managed state parent %q: %w", part, err)
		}
		fd = next
	}
	if err := unix.Fchmod(fd, 0o700); err != nil {
		_ = unix.Close(fd)
		return -1, fmt.Errorf("secure pinned state directory: %w", err)
	}
	return fd, nil
}

func pinnedDirOpenError(label string, err error) error {
	if errors.Is(err, unix.ENOTDIR) || errors.Is(err, unix.ELOOP) {
		return fmt.Errorf("%s is not a real directory", label)
	}
	return fmt.Errorf("pin %s: %w", label, err)
}
