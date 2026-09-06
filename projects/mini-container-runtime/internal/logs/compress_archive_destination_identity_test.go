package logs

import (
	"os"
	"path/filepath"
	"testing"
)

func TestCompressRotatedLogRejectsReplacedDestinationBeforeSourceRemoval(t *testing.T) {
	tmpDir := t.TempDir()
	logPath := filepath.Join(tmpDir, "container.log.1")
	gzPath := logPath + ".gz"
	if err := os.WriteFile(logPath, []byte("log data content"), 0644); err != nil {
		t.Fatal(err)
	}

	originalClose := compressArchiveFileClose
	defer func() { compressArchiveFileClose = originalClose }()

	replaced := false
	compressArchiveFileClose = func(f *os.File) error {
		if !replaced && f.Name() == gzPath {
			if err := f.Close(); err != nil {
				return err
			}
			if err := os.Rename(gzPath, gzPath+".original"); err != nil {
				return err
			}
			if err := os.WriteFile(gzPath, []byte("replacement"), 0644); err != nil {
				return err
			}
			replaced = true
			return nil
		}
		return f.Close()
	}

	if err := CompressRotatedLog(logPath); err == nil {
		t.Fatal("expected replaced gzip destination to be rejected")
	}
	if _, err := os.Stat(logPath); err != nil {
		t.Fatalf("source log must remain after destination replacement: %v", err)
	}
	got, err := os.ReadFile(gzPath)
	if err != nil {
		t.Fatal(err)
	}
	if string(got) != "replacement" {
		t.Fatalf("replacement destination was modified: %q", got)
	}
}
