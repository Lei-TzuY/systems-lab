//go:build linux

package image

import (
	"archive/tar"
	"crypto/rand"
	"encoding/hex"
	"errors"
	"fmt"
	"os"
	"time"

	"golang.org/x/sys/unix"
)

type hardlinkAtFunc func(olddirfd int, oldpath string, newdirfd int, newpath string, flags int) error
type hardlinkRenameAtFunc func(olddirfd int, oldpath string, newdirfd int, newpath string) error
type hardlinkStagingNameFunc func() (string, error)

func createHardlinkSecure(target, destDir, linkTarget string) error {
	return createHardlinkSecureWithHeader(target, destDir, linkTarget, nil, nil)
}

func createTarHardlinkSecure(target, destDir, linkTarget string, hdr *tar.Header) error {
	return createHardlinkSecureWithHeader(target, destDir, linkTarget, hdr, nil)
}

func createHardlinkSecureWithHook(target, destDir, linkTarget string, beforeLink func()) error {
	return createHardlinkSecureWithHeader(target, destDir, linkTarget, nil, beforeLink)
}

func createHardlinkSecureWithHeader(target, destDir, linkTarget string, hdr *tar.Header, beforeLink func()) error {
	root, err := openExtractionRoot(destDir)
	if err != nil {
		return err
	}
	defer root.Close()

	sourceParent, err := root.openParent(linkTarget, "hardlink source", false)
	if err != nil {
		return fmt.Errorf("open hardlink source parent: %w", err)
	}
	defer sourceParent.Close()

	// Pin and validate the exact source inode before opening the destination
	// parent with create=true. A missing or invalid source is a pure observation:
	// it must not create destination directories that can perturb later archive
	// entries while this hardlink waits for deferred resolution.
	sourceFD, err := unix.Openat(sourceParent.fd, sourceParent.leaf, unix.O_PATH|unix.O_CLOEXEC|unix.O_NOFOLLOW, 0)
	if err != nil {
		return fmt.Errorf("pin hardlink source %s: %w", linkTarget, err)
	}
	defer unix.Close(sourceFD)

	var sourceStat unix.Stat_t
	if err := unix.Fstat(sourceFD, &sourceStat); err != nil {
		return fmt.Errorf("inspect pinned hardlink source %s: %w", linkTarget, err)
	}
	if sourceStat.Mode&unix.S_IFMT == unix.S_IFDIR {
		return fmt.Errorf("refuse hardlink to directory %s", linkTarget)
	}
	if hdr != nil {
		if err := verifyPinnedHardlinkMetadata(sourceStat, hdr, os.Geteuid()); err != nil {
			return fmt.Errorf("validate hardlink metadata for %s: %w", linkTarget, err)
		}
		if err := verifyDeclaredXattrsPinnedFD(sourceFD, sourceStat.Mode, linkTarget, tarXattrsPortable(hdr)); err != nil {
			return fmt.Errorf("validate hardlink xattrs for %s: %w", linkTarget, err)
		}
	}

	destParent, err := root.openParent(target, "hardlink destination", true)
	if err != nil {
		return fmt.Errorf("open hardlink destination parent: %w", err)
	}
	defer destParent.Close()

	if beforeLink != nil {
		beforeLink()
	}

	// A directory destination cannot be replaced by renameat and is explicitly
	// outside the extractor's replacement contract. Other existing leaf types
	// are left untouched until an exact-source staging hardlink exists, so a
	// link failure cannot destroy the previous destination.
	var destStat unix.Stat_t
	statErr := unix.Fstatat(destParent.fd, destParent.leaf, &destStat, unix.AT_SYMLINK_NOFOLLOW)
	if statErr == nil {
		if destStat.Mode&unix.S_IFMT == unix.S_IFDIR {
			return fmt.Errorf("refuse to replace directory %s with hardlink", target)
		}
	} else if !errors.Is(statErr, unix.ENOENT) {
		return fmt.Errorf("inspect hardlink destination %s: %w", target, statErr)
	}

	if err := publishPinnedHardlinkSource(
		sourceFD,
		sourceStat.Mode,
		destParent.fd,
		destParent.leaf,
		unix.Linkat,
		unix.Renameat,
		newHardlinkStagingLeaf,
	); err != nil {
		return fmt.Errorf("hardlink pinned source %s → %s: %w", target, linkTarget, err)
	}
	return nil
}

func verifyPinnedHardlinkMetadata(source unix.Stat_t, hdr *tar.Header, euid int) error {
	if hdr == nil {
		return nil
	}
	expectedMode := uint32(hdr.Mode) & 0o7777
	actualMode := source.Mode & 0o7777
	if actualMode != expectedMode {
		return fmt.Errorf("declared mode %#o conflicts with source inode mode %#o", expectedMode, actualMode)
	}
	// Root extraction is expected to preserve archive ownership exactly. A
	// rootless extractor intentionally degrades EPERM chown to caller ownership,
	// so comparing the on-disk uid/gid there would reject otherwise supported
	// rootless archives even when both tar headers declare identical ownership.
	if euid == 0 && (source.Uid != uint32(hdr.Uid) || source.Gid != uint32(hdr.Gid)) {
		return fmt.Errorf("declared ownership %d:%d conflicts with source inode ownership %d:%d", hdr.Uid, hdr.Gid, source.Uid, source.Gid)
	}
	if !hdr.ModTime.IsZero() {
		actualMtime := time.Unix(source.Mtim.Sec, source.Mtim.Nsec)
		if !actualMtime.Equal(hdr.ModTime) {
			return fmt.Errorf("declared mtime %s conflicts with source inode mtime %s", hdr.ModTime.Format(time.RFC3339Nano), actualMtime.Format(time.RFC3339Nano))
		}
	}
	return nil
}

func newHardlinkStagingLeaf() (string, error) {
	var nonce [16]byte
	if _, err := rand.Read(nonce[:]); err != nil {
		return "", fmt.Errorf("generate hardlink staging name: %w", err)
	}
	return ".minicontainer-hardlink-" + hex.EncodeToString(nonce[:]), nil
}

func publishPinnedHardlinkSource(
	sourceFD int,
	sourceMode uint32,
	destParentFD int,
	destLeaf string,
	linkat hardlinkAtFunc,
	renameat hardlinkRenameAtFunc,
	stagingName hardlinkStagingNameFunc,
) error {
	if linkat == nil || renameat == nil || stagingName == nil {
		return fmt.Errorf("hardlink publish operation is nil")
	}

	const maxStagingAttempts = 16
	for attempt := 0; attempt < maxStagingAttempts; attempt++ {
		stagingLeaf, err := stagingName()
		if err != nil {
			return err
		}
		if stagingLeaf == "" || stagingLeaf == "." || stagingLeaf == ".." {
			return fmt.Errorf("invalid hardlink staging leaf %q", stagingLeaf)
		}

		if err := linkPinnedHardlinkSource(sourceFD, sourceMode, destParentFD, stagingLeaf, linkat); err != nil {
			if errors.Is(err, unix.EEXIST) {
				continue
			}
			return fmt.Errorf("stage pinned hardlink: %w", err)
		}

		if err := renameat(destParentFD, stagingLeaf, destParentFD, destLeaf); err != nil {
			publishErr := fmt.Errorf("publish staged hardlink over destination: %w", err)
			if cleanupErr := unix.Unlinkat(destParentFD, stagingLeaf, 0); cleanupErr != nil && !errors.Is(cleanupErr, unix.ENOENT) {
				return errors.Join(publishErr, fmt.Errorf("remove failed hardlink staging leaf %q: %w", stagingLeaf, cleanupErr))
			}
			return publishErr
		}
		return nil
	}
	return fmt.Errorf("allocate hardlink staging leaf after %d collisions", maxStagingAttempts)
}

func linkPinnedHardlinkSource(sourceFD int, sourceMode uint32, destParentFD int, destLeaf string, linkat hardlinkAtFunc) error {
	if linkat == nil {
		return fmt.Errorf("hardlink operation is nil")
	}

	// AT_EMPTY_PATH is the only fd-native link operation here: it names the
	// exact inode already proven above, including when that inode is itself a
	// symlink. It can require CAP_DAC_READ_SEARCH, so ordinary rootless callers
	// may need the procfs fallback below for non-symlink inodes.
	if err := linkat(sourceFD, "", destParentFD, destLeaf, unix.AT_EMPTY_PATH); err == nil {
		return nil
	} else if !errors.Is(err, unix.EPERM) && !errors.Is(err, unix.EINVAL) && !errors.Is(err, unix.ENOENT) {
		return err
	} else if sourceMode&unix.S_IFMT == unix.S_IFLNK {
		// /proc/self/fd/<fd> is itself a symlink. Using AT_SYMLINK_FOLLOW on
		// that path is a valid AT_EMPTY_PATH substitute for ordinary files, but
		// for an O_PATH|O_NOFOLLOW descriptor that pins a symlink inode it would
		// continue through the pinned symlink and link its target instead. That
		// silently changes archive semantics, so fail closed rather than
		// dereference a hardlink-to-symlink source.
		return fmt.Errorf("fd-native hardlink for symlink source unavailable: %w", err)
	}

	procSource := fmt.Sprintf("/proc/self/fd/%d", sourceFD)
	if err := linkat(unix.AT_FDCWD, procSource, destParentFD, destLeaf, unix.AT_SYMLINK_FOLLOW); err != nil {
		return fmt.Errorf("link pinned source via fd path: %w", err)
	}
	return nil
}
