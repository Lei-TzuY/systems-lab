package logs

import (
	"os"
	"path/filepath"
	"testing"
)

func TestCompressRotatedLogRejectsGroupWritableDestinationBeforeTruncate(t *testing.T) {
	tmpDir := t.TempDir()
	logPath := filepath.Join(tmpDir, "container.log.1")
	gzPath := logPath + ".gz"

	if err := os.WriteFile(logPath, []byte("new log data\n"), 0644); err != nil {
		t.Fatal(err)
	}
	const existing = "existing archive\n"
	if err := os.WriteFile(gzPath, []byte(existing), 0660); err != nil {
		t.Fatal(err)
	}
	if err := os.Chmod(gzPath, 0660); err != nil {
		t.Fatal(err)
	}

	if err := CompressRotatedLog(logPath); err == nil {
		t.Fatal("expected group-writable gzip destination to be rejected")
	}

	got, err := os.ReadFile(gzPath)
	if err != nil {
		t.Fatal(err)
	}
	if string(got) != existing {
		t.Fatalf("gzip destination was modified: got %q, want %q", got, existing)
	}
	if _, err := os.Stat(logPath); err != nil {
		t.Fatalf("source log should be retained: %v", err)
	}
}

func TestCompressRotatedLogRejectsDestinationMadeGroupWritableBeforeSourceRemoval(t *testing.T) {
	tmpDir := t.TempDir()
	logPath := filepath.Join(tmpDir, "container.log.1")
	gzPath := logPath + ".gz"

	if err := os.WriteFile(logPath, []byte("new log data\n"), 0644); err != nil {
		t.Fatal(err)
	}

	oldSync := compressArchiveSync
	compressArchiveSync = func(f *os.File) error {
		if err := f.Chmod(0660); err != nil {
			return err
		}
		return oldSync(f)
	}
	t.Cleanup(func() { compressArchiveSync = oldSync })

	if err := CompressRotatedLog(logPath); err == nil {
		t.Fatal("expected gzip destination made group-writable during compression to be rejected")
	}

	info, err := os.Stat(gzPath)
	if err != nil {
		t.Fatal(err)
	}
	if got := info.Mode().Perm(); got != 0660 {
		t.Fatalf("gzip destination mode = %v, want 0660", got)
	}
	if _, err := os.Stat(logPath); err != nil {
		t.Fatalf("source log should be retained: %v", err)
	}
}
