//go:build linux

package image

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"

	"golang.org/x/sys/unix"
)

// openDockerSaveMember opens one regular-file member beneath root without ever
// following an archive-controlled symlink component. The returned descriptor is
// bound to the exact inode selected during the dirfd walk.
func openDockerSaveMember(root, member string) (*os.File, error) {
	memberPath, err := safePath(root, member)
	if err != nil {
		return nil, err
	}
	rootAbs, err := filepath.Abs(root)
	if err != nil {
		return nil, fmt.Errorf("resolve docker-save root %q: %w", root, err)
	}
	rootAbs = filepath.Clean(rootAbs)
	memberAbs, err := filepath.Abs(memberPath)
	if err != nil {
		return nil, fmt.Errorf("resolve docker-save member %q: %w", member, err)
	}
	rel, err := filepath.Rel(rootAbs, filepath.Clean(memberAbs))
	if err != nil || rel == "." || rel == ".." || strings.HasPrefix(rel, ".."+string(filepath.Separator)) {
		return nil, fmt.Errorf("docker-save member %q escapes extraction root", member)
	}

	rootFD, err := unix.Open(rootAbs, unix.O_PATH|unix.O_DIRECTORY|unix.O_CLOEXEC|unix.O_NOFOLLOW, 0)
	if err != nil {
		return nil, fmt.Errorf("pin docker-save extraction root %q: %w", rootAbs, err)
	}
	parentFD := rootFD
	defer func() {
		if parentFD >= 0 {
			_ = unix.Close(parentFD)
		}
	}()

	parts := strings.Split(rel, string(filepath.Separator))
	for i, part := range parts {
		if part == "" || part == "." || part == ".." {
			return nil, fmt.Errorf("invalid docker-save member component %q in %q", part, member)
		}
		final := i == len(parts)-1
		if !final {
			fd, err := unix.Openat(parentFD, part, unix.O_PATH|unix.O_DIRECTORY|unix.O_CLOEXEC|unix.O_NOFOLLOW, 0)
			if err != nil {
				return nil, fmt.Errorf("pin docker-save member directory %q without following symlinks: %w", part, err)
			}
			_ = unix.Close(parentFD)
			parentFD = fd
			continue
		}

		fd, err := unix.Openat(parentFD, part, unix.O_RDONLY|unix.O_CLOEXEC|unix.O_NONBLOCK|unix.O_NOFOLLOW, 0)
		if err != nil {
			return nil, fmt.Errorf("open docker-save member %q without following symlinks: %w", member, err)
		}
		var st unix.Stat_t
		if err := unix.Fstat(fd, &st); err != nil {
			_ = unix.Close(fd)
			return nil, fmt.Errorf("inspect docker-save member %q: %w", member, err)
		}
		if st.Mode&unix.S_IFMT != unix.S_IFREG {
			_ = unix.Close(fd)
			return nil, fmt.Errorf("docker-save member %q is not a regular file", member)
		}
		return os.NewFile(uintptr(fd), memberAbs), nil
	}
	return nil, fmt.Errorf("docker-save member %q has no path components", member)
}
