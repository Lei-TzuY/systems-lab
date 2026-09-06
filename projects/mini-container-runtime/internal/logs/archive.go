package logs

import (
	"fmt"
	"os"
	"path/filepath"
	"time"

	"golang.org/x/sys/unix"
)

var archiveLstat = os.Lstat
var archiveSyncDir = syncArchiveDirectory

func syncArchiveDirectory(dir string) error {
	f, err := os.Open(dir)
	if err != nil {
		return fmt.Errorf("open log archive directory %q for fsync: %w", dir, err)
	}
	defer f.Close()
	if err := f.Sync(); err != nil {
		return fmt.Errorf("fsync log archive directory %q: %w", dir, err)
	}
	return nil
}

func renameArchiveNoReplace(src, dst string) error {
	return unix.Renameat2(unix.AT_FDCWD, src, unix.AT_FDCWD, dst, unix.RENAME_NOREPLACE)
}

func inspectArchiveFile(p string) (os.FileInfo, bool, error) {
	fi, err := archiveLstat(p)
	if err != nil {
		if os.IsNotExist(err) {
			return nil, false, nil
		}
		return nil, false, fmt.Errorf("inspect archived log %q: %w", p, err)
	}
	if !fi.Mode().IsRegular() {
		return nil, false, fmt.Errorf("unsafe archived log path %q: mode %v", p, fi.Mode())
	}
	nlink, err := fileInfoLinkCount(fi)
	if err != nil {
		return nil, false, fmt.Errorf("inspect archived log link count %q: %w", p, err)
	}
	if nlink != 1 {
		return nil, false, fmt.Errorf("unsafe archived log path %q: link count %d", p, nlink)
	}
	return fi, true, nil
}

func revalidateArchiveFile(p string, inspected os.FileInfo) (os.FileInfo, error) {
	current, err := archiveLstat(p)
	if err != nil {
		return nil, fmt.Errorf("revalidate archived log %q: %w", p, err)
	}
	if !current.Mode().IsRegular() {
		return nil, fmt.Errorf("unsafe archived log path %q during revalidation: mode %v", p, current.Mode())
	}
	if !os.SameFile(inspected, current) {
		return nil, fmt.Errorf("archived log path %q changed identity before rotation", p)
	}
	nlink, err := fileInfoLinkCount(current)
	if err != nil {
		return nil, fmt.Errorf("revalidate archived log link count %q: %w", p, err)
	}
	if nlink != 1 {
		return nil, fmt.Errorf("archived log path %q gained hard links before rotation (link count %d)", p, nlink)
	}
	return current, nil
}

func removeExpiredArchive(src string, inspected os.FileInfo) error {
	return removeExpiredArchiveBefore(src, inspected, time.Time{})
}

func removeExpiredArchiveBefore(src string, inspected os.FileInfo, cutoff time.Time) error {
	if !cutoff.IsZero() {
		current, err := revalidateArchiveFile(src, inspected)
		if err != nil {
			return err
		}
		sameCTime, err := fileInfoSameCTime(inspected, current)
		if err != nil {
			return fmt.Errorf("revalidate archived log change time %q before removal: %w", src, err)
		}
		if !sameCTime {
			return fmt.Errorf("archived log %q changed after prune age check", src)
		}
		if !current.ModTime().Before(cutoff) {
			return fmt.Errorf("archived log %q became fresh before removal", src)
		}
	}

	dir := filepath.Dir(src)
	placeholder, err := os.CreateTemp(dir, "."+filepath.Base(src)+".delete-*")
	if err != nil {
		return fmt.Errorf("reserve expired archived log tombstone for %q: %w", src, err)
	}
	tombstone := placeholder.Name()
	if err := placeholder.Close(); err != nil {
		_ = os.Remove(tombstone)
		return fmt.Errorf("close expired archived log tombstone placeholder %q: %w", tombstone, err)
	}
	if err := os.Remove(tombstone); err != nil {
		return fmt.Errorf("release expired archived log tombstone placeholder %q: %w", tombstone, err)
	}

	if err := renameArchiveNoReplace(src, tombstone); err != nil {
		return fmt.Errorf("stage expired archived log %q for removal without replacement: %w", src, err)
	}
	current, err := revalidateArchiveFile(tombstone, inspected)
	if err == nil && !cutoff.IsZero() && !current.ModTime().Before(cutoff) {
		err = fmt.Errorf("archived log %q became fresh before removal", src)
	}
	if err != nil {
		if restoreErr := renameArchiveNoReplace(tombstone, src); restoreErr != nil {
			return fmt.Errorf("%w; additionally failed to restore staged expired archived log %q to %q: %v", err, tombstone, src, restoreErr)
		}
		return err
	}
	if err := os.Remove(tombstone); err != nil {
		return fmt.Errorf("remove staged expired archived log %q: %w", tombstone, err)
	}
	return nil
}

// ArchiveLogFile shifts old log files (e.g. log.1 -> log.2) up to maxFiles.
func ArchiveLogFile(logPath string, maxFiles int) error {
	if maxFiles <= 1 {
		return nil
	}

	dir := filepath.Dir(logPath)
	for i := maxFiles - 1; i >= 1; i-- {
		src := fmt.Sprintf("%s.%d", logPath, i)
		dst := fmt.Sprintf("%s.%d", logPath, i+1)
		inspected, exists, err := inspectArchiveFile(src)
		if err != nil {
			return err
		}
		if exists {
			if _, err := revalidateArchiveFile(src, inspected); err != nil {
				return err
			}
			if i+1 >= maxFiles {
				if err := removeExpiredArchive(src, inspected); err != nil {
					return err
				}
				if err := archiveSyncDir(dir); err != nil {
					return fmt.Errorf("persist expired archived log removal %q: %w", src, err)
				}
			} else {
				if err := renameArchiveNoReplace(src, dst); err != nil {
					return fmt.Errorf("rotate archived log %q to %q without replacement: %w", src, dst, err)
				}
				if err := archiveSyncDir(dir); err != nil {
					return fmt.Errorf("persist archived log rotation %q to %q: %w", src, dst, err)
				}
			}
		}
	}

	inspected, exists, err := inspectArchiveFile(logPath)
	if err != nil {
		return err
	}
	if exists {
		if _, err := revalidateArchiveFile(logPath, inspected); err != nil {
			return err
		}
		dst := fmt.Sprintf("%s.1", logPath)
		if err := renameArchiveNoReplace(logPath, dst); err != nil {
			return fmt.Errorf("archive active log %q to %q without replacement: %w", logPath, dst, err)
		}
		if err := archiveSyncDir(dir); err != nil {
			return fmt.Errorf("persist active log archive %q to %q: %w", logPath, dst, err)
		}
	}

	return nil
}
