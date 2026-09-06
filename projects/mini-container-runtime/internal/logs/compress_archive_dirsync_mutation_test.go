package logs

import (
	"os"
	"path/filepath"
	"testing"
)

func TestCompressRotatedLogRejectsArchiveMutationDuringDirectorySync(t *testing.T) {
	tmpDir := t.TempDir()
	logPath := filepath.Join(tmpDir, "container.log.1")
	gzPath := logPath + ".gz"
	original := []byte("original log data\n")
	if err := os.WriteFile(logPath, original, 0644); err != nil {
		t.Fatal(err)
	}

	oldSyncDir := compressArchiveSyncDir
	calls := 0
	compressArchiveSyncDir = func(dir string) error {
		calls++
		if err := oldSyncDir(dir); err != nil {
			return err
		}
		if calls != 1 {
			return nil
		}
		f, err := os.OpenFile(gzPath, os.O_WRONLY|os.O_APPEND, 0)
		if err != nil {
			return err
		}
		if _, err := f.Write([]byte("tampered")); err != nil {
			_ = f.Close()
			return err
		}
		return f.Close()
	}
	t.Cleanup(func() { compressArchiveSyncDir = oldSyncDir })

	if err := CompressRotatedLog(logPath); err == nil {
		t.Fatal("expected archive mutation during directory sync to be rejected")
	}
	got, err := os.ReadFile(logPath)
	if err != nil {
		t.Fatalf("source log should be retained: %v", err)
	}
	if string(got) != string(original) {
		t.Fatalf("source log content = %q, want %q", got, original)
	}
}
