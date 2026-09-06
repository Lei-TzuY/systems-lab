package logs

import (
	"os"
	"path/filepath"
	"testing"
)

func TestCompressRotatedLogRejectsSourceAppendedBeforeRemoval(t *testing.T) {
	tmpDir := t.TempDir()
	logPath := filepath.Join(tmpDir, "container.log.1")
	if err := os.WriteFile(logPath, []byte("original log data\n"), 0644); err != nil {
		t.Fatal(err)
	}

	oldSyncDir := compressArchiveSyncDir
	calls := 0
	compressArchiveSyncDir = func(dir string) error {
		calls++
		if calls == 1 {
			f, err := os.OpenFile(logPath, os.O_WRONLY|os.O_APPEND, 0)
			if err != nil {
				return err
			}
			if _, err := f.WriteString("late log data\n"); err != nil {
				_ = f.Close()
				return err
			}
			if err := f.Close(); err != nil {
				return err
			}
		}
		return oldSyncDir(dir)
	}
	t.Cleanup(func() { compressArchiveSyncDir = oldSyncDir })

	if err := CompressRotatedLog(logPath); err == nil {
		t.Fatal("expected source appended after compression to be rejected")
	}

	got, err := os.ReadFile(logPath)
	if err != nil {
		t.Fatalf("source log should be retained: %v", err)
	}
	if string(got) != "original log data\nlate log data\n" {
		t.Fatalf("source log content = %q, want appended data preserved", got)
	}
}
