package logs

import (
	"os"
	"path/filepath"
	"testing"
)

func TestCompressRotatedLogRejectsSameSizeSourceRewriteWithRestoredMtime(t *testing.T) {
	tmpDir := t.TempDir()
	logPath := filepath.Join(tmpDir, "container.log.1")
	original := []byte("original log data\n")
	if err := os.WriteFile(logPath, original, 0644); err != nil {
		t.Fatal(err)
	}
	initialInfo, err := os.Stat(logPath)
	if err != nil {
		t.Fatal(err)
	}

	oldSyncDir := compressArchiveSyncDir
	calls := 0
	compressArchiveSyncDir = func(dir string) error {
		calls++
		if calls == 1 {
			replacement := []byte("rewritten log dat\n")
			if len(replacement) != len(original) {
				t.Fatalf("test replacement length = %d, want %d", len(replacement), len(original))
			}
			if err := os.WriteFile(logPath, replacement, 0644); err != nil {
				return err
			}
			if err := os.Chtimes(logPath, initialInfo.ModTime(), initialInfo.ModTime()); err != nil {
				return err
			}
		}
		return oldSyncDir(dir)
	}
	t.Cleanup(func() { compressArchiveSyncDir = oldSyncDir })

	if err := CompressRotatedLog(logPath); err == nil {
		t.Fatal("expected same-size source rewrite with restored mtime to be rejected")
	}

	got, err := os.ReadFile(logPath)
	if err != nil {
		t.Fatalf("source log should be retained: %v", err)
	}
	if string(got) != "rewritten log dat\n" {
		t.Fatalf("source log content = %q, want rewritten data preserved", got)
	}
}
