//go:build linux

package image

import (
	"errors"
	"fmt"
	"os"

	"golang.org/x/sys/unix"
)

func createDirectorySecure(target, destDir string, mode os.FileMode) error {
	return createDirectorySecureWithHook(target, destDir, mode, nil)
}

func createDirectorySecureWithHook(target, destDir string, mode os.FileMode, beforeCreate func()) error {
	root, err := openExtractionRoot(destDir)
	if err != nil {
		return err
	}
	defer root.Close()
	parent, err := root.openParent(target, "directory", true)
	if err != nil {
		return err
	}
	defer parent.Close()
	if beforeCreate != nil {
		beforeCreate()
	}

	var st unix.Stat_t
	statErr := unix.Fstatat(parent.fd, parent.leaf, &st, unix.AT_SYMLINK_NOFOLLOW)
	if statErr == nil {
		switch st.Mode & unix.S_IFMT {
		case unix.S_IFDIR:
			return nil
		case unix.S_IFLNK:
			if err := unix.Unlinkat(parent.fd, parent.leaf, 0); err != nil {
				return fmt.Errorf("remove existing symlink before mkdir %s: %w", target, err)
			}
		default:
			return fmt.Errorf("refuse to replace non-directory %s with directory", target)
		}
	} else if !errors.Is(statErr, unix.ENOENT) {
		return fmt.Errorf("inspect directory target %s: %w", target, statErr)
	}

	if err := unix.Mkdirat(parent.fd, parent.leaf, uint32(mode.Perm())); err != nil {
		return fmt.Errorf("mkdir %s relative to pinned parent: %w", target, err)
	}
	return nil
}
