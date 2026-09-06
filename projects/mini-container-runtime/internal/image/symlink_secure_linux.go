//go:build linux

package image

import (
	"archive/tar"
	"errors"
	"fmt"
	"os"
	"time"

	"golang.org/x/sys/unix"
)

func createSymlinkSecure(target, destDir, linkname string) error {
	return createSymlinkSecureWithHook(target, destDir, linkname, nil)
}

func createSymlinkSecureWithHook(target, destDir, linkname string, beforeCreate func()) error {
	return createSymlinkSecureInternal(target, destDir, linkname, nil, beforeCreate)
}

// createTarSymlinkSecure restores symlink inode metadata while the exact
// extraction parent is still pinned. Ownership and timestamps use
// AT_SYMLINK_NOFOLLOW so an archive link can never redirect metadata writes to
// its target.
func createTarSymlinkSecure(target, destDir string, hdr *tar.Header) error {
	if hdr == nil {
		return fmt.Errorf("symlink tar header is nil")
	}
	return createSymlinkSecureInternal(target, destDir, hdr.Linkname, hdr, nil)
}

func createSymlinkSecureInternal(target, destDir, linkname string, hdr *tar.Header, beforeCreate func()) error {
	root, err := openExtractionRoot(destDir)
	if err != nil {
		return err
	}
	defer root.Close()
	parent, err := root.openParent(target, "symlink", true)
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
		if st.Mode&unix.S_IFMT == unix.S_IFDIR {
			return fmt.Errorf("refuse to replace directory %s with symlink", target)
		}
		if err := unix.Unlinkat(parent.fd, parent.leaf, 0); err != nil {
			return fmt.Errorf("unlink existing symlink target %s: %w", target, err)
		}
	} else if !errors.Is(statErr, unix.ENOENT) {
		return fmt.Errorf("inspect symlink target %s: %w", target, statErr)
	}

	if err := unix.Symlinkat(linkname, parent.fd, parent.leaf); err != nil {
		return fmt.Errorf("symlink %s → %s relative to pinned parent: %w", target, linkname, err)
	}
	if hdr == nil {
		return nil
	}
	if err := restoreSymlinkOwnership(parent.fd, parent.leaf, target, hdr.Uid, hdr.Gid); err != nil {
		return err
	}
	if !hdr.ModTime.IsZero() {
		times := []unix.Timespec{
			unix.NsecToTimespec(time.Now().UnixNano()),
			unix.NsecToTimespec(hdr.ModTime.UnixNano()),
		}
		if err := unix.UtimesNanoAt(parent.fd, parent.leaf, times, unix.AT_SYMLINK_NOFOLLOW); err != nil {
			return fmt.Errorf("restore symlink mtime on %s: %w", target, err)
		}
	}
	return nil
}

func restoreSymlinkOwnership(parentFD int, leaf, target string, uid, gid int) error {
	if uid < 0 || gid < 0 {
		return fmt.Errorf("invalid negative tar ownership %d:%d", uid, gid)
	}
	if err := unix.Fchownat(parentFD, leaf, uid, gid, unix.AT_SYMLINK_NOFOLLOW); err != nil {
		// Match regular/directory extraction semantics: rootless callers cannot
		// represent arbitrary archive ownership, but privileged failures remain
		// fatal and must never be hidden.
		if errors.Is(err, unix.EPERM) && os.Geteuid() != 0 {
			return nil
		}
		return fmt.Errorf("restore symlink ownership %d:%d on %s: %w", uid, gid, target, err)
	}
	return nil
}
