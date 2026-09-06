package logs

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"time"
)

var pruneBeforeInfo = func(string) {}
var pruneBeforeDelete = func(string) {}

func isRotatedLogName(name string) bool {
	marker := strings.LastIndex(name, ".log.")
	if marker < 0 {
		return false
	}

	suffix := name[marker+len(".log."):]
	if strings.HasSuffix(suffix, ".gz") {
		suffix = strings.TrimSuffix(suffix, ".gz")
	}
	if suffix == "" {
		return false
	}
	for _, r := range suffix {
		if r < '0' || r > '9' {
			return false
		}
	}
	return true
}

// PruneRotatedLogs deletes rotated log files older than maxAge duration.
func PruneRotatedLogs(logDir string, maxAge time.Duration) (int, error) {
	if maxAge <= 0 {
		return 0, nil
	}

	entries, err := os.ReadDir(logDir)
	if err != nil {
		if os.IsNotExist(err) {
			return 0, nil
		}
		return 0, fmt.Errorf("read log dir: %w", err)
	}

	cutoff := time.Now().Add(-maxAge)
	deletedCount := 0

	for _, entry := range entries {
		name := entry.Name()
		if isRotatedLogName(name) && !entry.IsDir() {
			path := filepath.Join(logDir, name)
			pruneBeforeInfo(path)
			fi, err := entry.Info()
			if err != nil {
				return deletedCount, fmt.Errorf("inspect rotated log %q: %w", path, err)
			}
			if fi.ModTime().Before(cutoff) {
				pruneBeforeDelete(path)
				if err := removeExpiredArchiveBefore(path, fi, cutoff); err != nil {
					return deletedCount, fmt.Errorf("prune rotated log %q: %w", path, err)
				}
				if err := archiveSyncDir(logDir); err != nil {
					return deletedCount, fmt.Errorf("persist pruned rotated log removal %q: %w", path, err)
				}
				deletedCount++
			}
		}
	}

	return deletedCount, nil
}
