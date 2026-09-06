//go:build linux

package image

import (
	"archive/tar"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"syscall"
	"time"

	"golang.org/x/sys/unix"
)

// makeSpecialSecure creates device nodes and FIFOs relative to a pinned
// extraction parent. Parent traversal uses O_NOFOLLOW so a concurrent
// rename/symlink replacement cannot redirect a privileged mknod outside the
// extraction root.
func makeSpecialSecure(target, destDir string, hdr *tar.Header) error {
	return makeSpecialSecureWithHook(target, destDir, hdr, nil)
}

func makeSpecialSecureWithHook(target, destDir string, hdr *tar.Header, beforeCreate func()) error {
	return makeSpecialSecureWithHooks(target, destDir, hdr, beforeCreate, nil)
}

func makeSpecialSecureWithHooks(target, destDir string, hdr *tar.Header, beforeCreate, beforeMetadata func()) error {
	if hdr == nil {
		return fmt.Errorf("special tar header is nil")
	}
	root, err := openExtractionRoot(destDir)
	if err != nil {
		return err
	}
	defer root.Close()
	parent, err := root.openParent(target, "special", true)
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
			return fmt.Errorf("refuse to replace directory %s with special node", target)
		}
		if err := unix.Unlinkat(parent.fd, parent.leaf, 0); err != nil {
			return fmt.Errorf("unlink existing special target %s: %w", target, err)
		}
	} else if !errors.Is(statErr, unix.ENOENT) {
		return fmt.Errorf("inspect special target %s: %w", target, statErr)
	}

	mode, dev, err := specialModeDevice(hdr)
	if err != nil {
		return err
	}
	if err := unix.Mknodat(parent.fd, parent.leaf, mode, int(dev)); err != nil {
		return err
	}

	// Pin the exact inode without opening the device/FIFO itself. Subsequent
	// ownership, mode, xattr, and timestamp restoration is addressed through
	// this O_PATH handle rather than the archive pathname, so leaf replacement
	// cannot redirect metadata writes to a foreign inode.
	inodeFD, err := unix.Openat(parent.fd, parent.leaf, unix.O_PATH|unix.O_CLOEXEC|unix.O_NOFOLLOW, 0)
	if err != nil {
		return fmt.Errorf("pin special target %s: %w", target, err)
	}
	defer unix.Close(inodeFD)
	if beforeMetadata != nil {
		beforeMetadata()
	}
	if err := restoreSpecialMetadataFD(inodeFD, target, hdr); err != nil {
		return err
	}
	return nil
}

func restoreSpecialMetadataFD(fd int, target string, hdr *tar.Header) error {
	if err := restoreOwnershipWith(fd, hdr.Uid, hdr.Gid, os.Geteuid(), func(fd, uid, gid int) error {
		return unix.Fchownat(fd, "", uid, gid, unix.AT_EMPTY_PATH)
	}); err != nil {
		return fmt.Errorf("restore ownership %d:%d on special node %s: %w", hdr.Uid, hdr.Gid, target, err)
	}

	// Fchown may clear setuid/setgid bits. Re-apply the exact tar mode after
	// ownership using the kernel-controlled procfs path for the already pinned
	// O_PATH descriptor; this never re-resolves the archive pathname.
	fdPath := fmt.Sprintf("/proc/self/fd/%d", fd)
	if err := os.Chmod(fdPath, hdr.FileInfo().Mode()); err != nil {
		return fmt.Errorf("restore mode on special node %s: %w", target, err)
	}
	// Apply xattrs after ownership/mode. In particular, a later chown can clear
	// security.capability, so restoring capabilities earlier would silently lose
	// archive semantics.
	if err := restoreXattrsPinnedFD(fd, target, tarXattrsPortable(hdr)); err != nil {
		return err
	}
	if !hdr.ModTime.IsZero() {
		if err := os.Chtimes(fdPath, time.Now(), hdr.ModTime); err != nil {
			return fmt.Errorf("restore mtime on special node %s: %w", target, err)
		}
	}
	return nil
}

func specialModeDevice(hdr *tar.Header) (uint32, uint64, error) {
	mode := tarUnixMode(hdr.FileInfo().Mode())
	var dev uint64
	switch hdr.Typeflag {
	case tar.TypeChar:
		mode |= syscall.S_IFCHR
		dev = mkdev(uint(hdr.Devmajor), uint(hdr.Devminor))
	case tar.TypeBlock:
		mode |= syscall.S_IFBLK
		dev = mkdev(uint(hdr.Devmajor), uint(hdr.Devminor))
	case tar.TypeFifo:
		mode |= syscall.S_IFIFO
	default:
		return 0, 0, fmt.Errorf("unexpected type flag: %d", hdr.Typeflag)
	}
	return mode, dev, nil
}

// makeSpecial remains available as the portable pathname primitive for callers
// outside archive extraction. Production tar extraction uses makeSpecialSecure.
func makeSpecial(target string, hdr *tar.Header) error {
	if err := os.MkdirAll(filepath.Dir(target), 0o755); err != nil {
		return err
	}
	_ = os.Remove(target)
	mode, dev, err := specialModeDevice(hdr)
	if err != nil {
		return err
	}
	return syscall.Mknod(target, mode, int(dev))
}

// mkdev encodes a major/minor pair into a Linux device number.
// The encoding is: bits[19:8]=major, bits[7:0]=minor_low, bits[31:20]=minor_high.
func mkdev(major, minor uint) uint64 {
	return (uint64(major) << 8) |
		(uint64(minor) & 0xff) |
		((uint64(minor) &^ 0xff) << 12)
}
