package logs

import (
	"os"
	"path/filepath"
	"testing"
)

func TestCompressRotatedLogRejectsNonRegularSourceBeforeCreatingArchive(t *testing.T) {
	tmpDir := t.TempDir()
	logPath := filepath.Join(tmpDir, "container.log.1")
	if err := os.Mkdir(logPath, 0755); err != nil {
		t.Fatal(err)
	}

	if err := CompressRotatedLog(logPath); err == nil {
		t.Fatal("expected non-regular source to be rejected")
	}
	if fi, err := os.Stat(logPath); err != nil {
		t.Fatalf("non-regular source should remain untouched: %v", err)
	} else if !fi.IsDir() {
		t.Fatalf("source mode changed unexpectedly: %v", fi.Mode())
	}
	if _, err := os.Lstat(logPath + ".gz"); !os.IsNotExist(err) {
		t.Fatalf("gzip archive should not be created for non-regular source, lstat err = %v", err)
	}
}
