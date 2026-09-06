//go:build linux

package image

import (
	"bytes"
	"errors"
	"fmt"
	"sort"
	"strings"

	"golang.org/x/sys/unix"
)

func restoreXattrsWith(target string, xattrs map[string][]byte, set func(name string, value []byte) error) error {
	if len(xattrs) == 0 {
		return nil
	}
	names := make([]string, 0, len(xattrs))
	for name := range xattrs {
		names = append(names, name)
	}
	sort.Strings(names)
	for _, name := range names {
		if strings.IndexByte(name, 0) >= 0 {
			return fmt.Errorf("restore xattr on %s: invalid NUL in name", target)
		}
		if err := set(name, xattrs[name]); err != nil {
			return fmt.Errorf("restore xattr %q on %s: %w", name, target, err)
		}
	}
	return nil
}

func restoreXattrsFD(fd int, target string, xattrs map[string][]byte) error {
	return restoreXattrsWith(target, xattrs, func(name string, value []byte) error {
		return unix.Fsetxattr(fd, name, value, 0)
	})
}

// restoreXattrsPinnedFD restores xattrs through the kernel-owned procfs path
// for an already-pinned O_PATH descriptor. Fsetxattr rejects O_PATH FDs, while
// /proc/self/fd/<n> resolves to that exact inode rather than the archive
// pathname, so a concurrent rename/replacement cannot redirect metadata writes.
func restoreXattrsPinnedFD(fd int, target string, xattrs map[string][]byte) error {
	fdPath := fmt.Sprintf("/proc/self/fd/%d", fd)
	return restoreXattrsWith(target, xattrs, func(name string, value []byte) error {
		return unix.Setxattr(fdPath, name, value, 0)
	})
}

func readXattrPinnedFD(fd int, name string) ([]byte, error) {
	fdPath := fmt.Sprintf("/proc/self/fd/%d", fd)
	for attempt := 0; attempt < 2; attempt++ {
		size, err := unix.Getxattr(fdPath, name, nil)
		if err != nil {
			return nil, err
		}
		value := make([]byte, size)
		n, err := unix.Getxattr(fdPath, name, value)
		if errors.Is(err, unix.ERANGE) {
			continue
		}
		if err != nil {
			return nil, err
		}
		return value[:n], nil
	}
	return nil, unix.ERANGE
}

// verifyDeclaredXattrsPinnedFD checks only xattrs explicitly declared by an
// archive entry. Omitted xattrs are not treated as declarations of absence.
// The procfs path is safe for ordinary pinned inodes, but following it for an
// O_PATH descriptor that pins a symlink would continue through the symlink and
// inspect its target instead of the symlink inode, so that case fails closed.
func verifyDeclaredXattrsPinnedFD(fd int, sourceMode uint32, target string, declared map[string][]byte) error {
	if len(declared) == 0 {
		return nil
	}
	if sourceMode&unix.S_IFMT == unix.S_IFLNK {
		return fmt.Errorf("cannot safely verify declared xattrs on pinned symlink %s", target)
	}
	names := make([]string, 0, len(declared))
	for name := range declared {
		names = append(names, name)
	}
	sort.Strings(names)
	for _, name := range names {
		if strings.IndexByte(name, 0) >= 0 {
			return fmt.Errorf("verify xattr on %s: invalid NUL in name", target)
		}
		actual, err := readXattrPinnedFD(fd, name)
		if errors.Is(err, unix.ENODATA) {
			return fmt.Errorf("declared xattr %q is missing from source inode", name)
		}
		if err != nil {
			return fmt.Errorf("read declared xattr %q from %s: %w", name, target, err)
		}
		if !bytes.Equal(actual, declared[name]) {
			return fmt.Errorf("declared xattr %q conflicts with source inode value", name)
		}
	}
	return nil
}
