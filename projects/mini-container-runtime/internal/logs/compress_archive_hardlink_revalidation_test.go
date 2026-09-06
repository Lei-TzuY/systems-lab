package logs

import (
	"os"
	"path/filepath"
	"testing"
)

func TestCompressRotatedLogRejectsDestinationHardLinkedBeforeSourceRemoval(t *testing.T) {
	tmpDir := t.TempDir()
	logPath := filepath.Join(tmpDir, "container.log.1")
	gzPath := logPath + ".gz"
	linkPath := filepath.Join(tmpDir, "archive-link.gz")

	if err := os.WriteFile(logPath, []byte("new log data\n"), 0644); err != nil {
		t.Fatal(err)
	}

	oldSync := compressArchiveSync
	compressArchiveSync = func(f *os.File) error {
		if err := oldSync(f); err != nil {
			return err
		}
		if err := os.Link(gzPath, linkPath); err != nil {
			return err
		}
		return nil
	}
	t.Cleanup(func() { compressArchiveSync = oldSync })

	if err := CompressRotatedLog(logPath); err == nil {
		t.Fatal("expected gzip destination hard-linked during compression to be rejected")
	}

	info, err := os.Stat(gzPath)
	if err != nil {
		t.Fatal(err)
	}
	if info.Mode().Perm()&0022 != 0 {
		t.Fatalf("gzip destination unexpectedly writable by group or others: %v", info.Mode().Perm())
	}
	if _, err := os.Stat(linkPath); err != nil {
		t.Fatalf("expected hard link to exist: %v", err)
	}
	if _, err := os.Stat(logPath); err != nil {
		t.Fatalf("source log should be retained: %v", err)
	}
}

func TestCompressRotatedLogRejectsSourceHardLinkedBeforeRemoval(t *testing.T) {
	tmpDir := t.TempDir()
	logPath := filepath.Join(tmpDir, "container.log.1")
	linkPath := filepath.Join(tmpDir, "source-link.log")

	if err := os.WriteFile(logPath, []byte("new log data\n"), 0644); err != nil {
		t.Fatal(err)
	}

	oldSyncDir := compressArchiveSyncDir
	compressArchiveSyncDir = func(dir string) error {
		if err := oldSyncDir(dir); err != nil {
			return err
		}
		if _, err := os.Stat(linkPath); os.IsNotExist(err) {
			if err := os.Link(logPath, linkPath); err != nil {
				return err
			}
		}
		return nil
	}
	t.Cleanup(func() { compressArchiveSyncDir = oldSyncDir })

	if err := CompressRotatedLog(logPath); err == nil {
		t.Fatal("expected source hard-linked during compression to be rejected")
	}

	if _, err := os.Stat(logPath); err != nil {
		t.Fatalf("source log should be retained: %v", err)
	}
	if _, err := os.Stat(linkPath); err != nil {
		t.Fatalf("source hard link should remain: %v", err)
	}
}
