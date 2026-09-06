package logs

import (
	"compress/gzip"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"syscall"

	"golang.org/x/sys/unix"
)

var (
	compressArchiveRemove    = os.Remove
	compressArchiveGzipClose = func(w *gzip.Writer) error { return w.Close() }
	compressArchiveSync      = func(f *os.File) error { return f.Sync() }
	compressArchiveFileClose = func(f *os.File) error { return f.Close() }
	compressArchiveSyncDir   = syncArchiveDirectory
)

func fileInfoStat(info os.FileInfo) (*syscall.Stat_t, error) {
	stat, ok := info.Sys().(*syscall.Stat_t)
	if !ok {
		return nil, fmt.Errorf("unexpected stat payload %T", info.Sys())
	}
	return stat, nil
}

func fileInfoLinkCount(info os.FileInfo) (uint64, error) {
	stat, err := fileInfoStat(info)
	if err != nil {
		return 0, err
	}
	return uint64(stat.Nlink), nil
}

func fileInfoSameCTime(a, b os.FileInfo) (bool, error) {
	aStat, err := fileInfoStat(a)
	if err != nil {
		return false, err
	}
	bStat, err := fileInfoStat(b)
	if err != nil {
		return false, err
	}
	return aStat.Ctim.Sec == bStat.Ctim.Sec && aStat.Ctim.Nsec == bStat.Ctim.Nsec, nil
}

func revalidateCompressedArchive(gzPath string, openedInfo, durableInfo os.FileInfo) error {
	currentInfo, err := os.Lstat(gzPath)
	if err != nil {
		return fmt.Errorf("revalidate gzip archive %q: %w", gzPath, err)
	}
	if !currentInfo.Mode().IsRegular() || !os.SameFile(openedInfo, currentInfo) {
		return fmt.Errorf("gzip archive destination %q changed during compression", gzPath)
	}
	if currentInfo.Size() != durableInfo.Size() || !currentInfo.ModTime().Equal(durableInfo.ModTime()) {
		return fmt.Errorf("gzip archive destination %q content changed after sync", gzPath)
	}
	sameCTime, err := fileInfoSameCTime(durableInfo, currentInfo)
	if err != nil {
		return fmt.Errorf("revalidate gzip archive change time %q: %w", gzPath, err)
	}
	if !sameCTime {
		return fmt.Errorf("gzip archive destination %q metadata changed after sync", gzPath)
	}
	if currentInfo.Mode().Perm()&0022 != 0 {
		return fmt.Errorf("gzip archive destination %q became writable by group or others during compression (mode %v)", gzPath, currentInfo.Mode().Perm())
	}
	currentNlink, err := fileInfoLinkCount(currentInfo)
	if err != nil {
		return fmt.Errorf("revalidate gzip archive link count %q: %w", gzPath, err)
	}
	if currentNlink != 1 {
		return fmt.Errorf("gzip archive destination %q gained hard links during compression (link count %d)", gzPath, currentNlink)
	}
	return nil
}

// CompressRotatedLog compresses logPath to logPath.gz and removes the uncompressed file.
func CompressRotatedLog(logPath string) error {
	srcFile, err := os.OpenFile(logPath, os.O_RDONLY|unix.O_NOFOLLOW, 0)
	if err != nil {
		if os.IsNotExist(err) {
			return nil
		}
		return fmt.Errorf("open log file: %w", err)
	}
	defer srcFile.Close()

	srcInfo, err := srcFile.Stat()
	if err != nil {
		return fmt.Errorf("stat opened log file %q: %w", logPath, err)
	}
	if !srcInfo.Mode().IsRegular() {
		return fmt.Errorf("unsafe compressed source log %q: mode %v", logPath, srcInfo.Mode())
	}
	srcNlink, err := fileInfoLinkCount(srcInfo)
	if err != nil {
		return fmt.Errorf("stat compressed source log link count %q: %w", logPath, err)
	}
	if srcNlink != 1 {
		return fmt.Errorf("unsafe compressed source log %q: link count %d", logPath, srcNlink)
	}

	gzPath := logPath + ".gz"
	dstFile, err := os.OpenFile(gzPath, os.O_WRONLY|os.O_CREATE|unix.O_NOFOLLOW|unix.O_NONBLOCK, 0644)
	if err != nil {
		return fmt.Errorf("create gz file: %w", err)
	}
	defer dstFile.Close()

	dstInfo, err := dstFile.Stat()
	if err != nil {
		return fmt.Errorf("stat opened gzip archive %q: %w", gzPath, err)
	}
	if !dstInfo.Mode().IsRegular() {
		return fmt.Errorf("unsafe gzip archive destination %q: mode %v", gzPath, dstInfo.Mode())
	}
	if dstInfo.Mode().Perm()&0022 != 0 {
		return fmt.Errorf("unsafe gzip archive destination %q: writable by group or others (mode %v)", gzPath, dstInfo.Mode().Perm())
	}
	var dstStat unix.Stat_t
	if err := unix.Fstat(int(dstFile.Fd()), &dstStat); err != nil {
		return fmt.Errorf("fstat gzip archive %q: %w", gzPath, err)
	}
	if dstStat.Nlink != 1 {
		return fmt.Errorf("unsafe gzip archive destination %q: link count %d", gzPath, dstStat.Nlink)
	}
	if err := dstFile.Truncate(0); err != nil {
		return fmt.Errorf("truncate gzip archive %q: %w", gzPath, err)
	}
	if _, err := dstFile.Seek(0, io.SeekStart); err != nil {
		return fmt.Errorf("rewind gzip archive %q: %w", gzPath, err)
	}

	gzWriter := gzip.NewWriter(dstFile)

	if _, err := io.Copy(gzWriter, srcFile); err != nil {
		_ = gzWriter.Close()
		return fmt.Errorf("gzip compress: %w", err)
	}
	if err := compressArchiveGzipClose(gzWriter); err != nil {
		return fmt.Errorf("finalize gzip archive %q: %w", gzPath, err)
	}
	if err := compressArchiveSync(dstFile); err != nil {
		return fmt.Errorf("sync gzip archive %q: %w", gzPath, err)
	}
	durableDstInfo, err := dstFile.Stat()
	if err != nil {
		return fmt.Errorf("stat synced gzip archive %q: %w", gzPath, err)
	}
	if err := compressArchiveFileClose(dstFile); err != nil {
		return fmt.Errorf("close gzip archive %q: %w", gzPath, err)
	}

	if err := revalidateCompressedArchive(gzPath, dstInfo, durableDstInfo); err != nil {
		return err
	}
	archiveDir := filepath.Dir(logPath)
	if err := compressArchiveSyncDir(archiveDir); err != nil {
		return fmt.Errorf("persist gzip archive %q: %w", gzPath, err)
	}
	if err := revalidateCompressedArchive(gzPath, dstInfo, durableDstInfo); err != nil {
		return err
	}

	if err := srcFile.Close(); err != nil {
		return fmt.Errorf("close compressed source log %q: %w", logPath, err)
	}
	currentInfo, err := os.Lstat(logPath)
	if err != nil {
		return fmt.Errorf("revalidate compressed source log %q: %w", logPath, err)
	}
	if currentInfo.Mode()&os.ModeSymlink != 0 || !os.SameFile(srcInfo, currentInfo) {
		return fmt.Errorf("compressed source log %q changed during compression", logPath)
	}
	if currentInfo.Size() != srcInfo.Size() || !currentInfo.ModTime().Equal(srcInfo.ModTime()) {
		return fmt.Errorf("compressed source log %q content changed during compression", logPath)
	}
	sameCTime, err := fileInfoSameCTime(srcInfo, currentInfo)
	if err != nil {
		return fmt.Errorf("revalidate compressed source log change time %q: %w", logPath, err)
	}
	if !sameCTime {
		return fmt.Errorf("compressed source log %q metadata changed during compression", logPath)
	}
	currentNlink, err := fileInfoLinkCount(currentInfo)
	if err != nil {
		return fmt.Errorf("revalidate compressed source log link count %q: %w", logPath, err)
	}
	if currentNlink != 1 {
		return fmt.Errorf("compressed source log %q gained hard links during compression (link count %d)", logPath, currentNlink)
	}
	if err := compressArchiveRemove(logPath); err != nil {
		return fmt.Errorf("remove compressed source log %q: %w", logPath, err)
	}
	if err := compressArchiveSyncDir(archiveDir); err != nil {
		return fmt.Errorf("persist compressed source log removal %q: %w", logPath, err)
	}

	return nil
}
