package logs

import (
	"os"
	"path/filepath"
	"testing"
)

func TestCompressRotatedLogRejectsArchiveMutationAfterSync(t *testing.T) {
	tmpDir := t.TempDir()
	logPath := filepath.Join(tmpDir, "container.log.1")
	gzPath := logPath + ".gz"
	original := []byte("original log data\n")
	if err := os.WriteFile(logPath, original, 0644); err != nil {
		t.Fatal(err)
	}

	oldClose := compressArchiveFileClose
	compressArchiveFileClose = func(f *os.File) error {
		if err := oldClose(f); err != nil {
			return err
		}
		info, err := os.Stat(gzPath)
		if err != nil {
			return err
		}
		data, err := os.ReadFile(gzPath)
		if err != nil {
			return err
		}
		if len(data) == 0 {
			t.Fatal("gzip archive unexpectedly empty")
		}
		data[0] ^= 0xff
		if err := os.WriteFile(gzPath, data, info.Mode().Perm()); err != nil {
			return err
		}
		return os.Chtimes(gzPath, info.ModTime(), info.ModTime())
	}
	t.Cleanup(func() { compressArchiveFileClose = oldClose })

	if err := CompressRotatedLog(logPath); err == nil {
		t.Fatal("expected post-sync archive mutation to be rejected")
	}
	got, err := os.ReadFile(logPath)
	if err != nil {
		t.Fatalf("source log should be retained: %v", err)
	}
	if string(got) != string(original) {
		t.Fatalf("source log content = %q, want %q", got, original)
	}
}
