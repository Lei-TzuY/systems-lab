package logs

import (
	"os"
	"path/filepath"
	"testing"
)

func TestArchiveLogFileDoesNotRemoveExpiredReplacementAfterRevalidation(t *testing.T) {
	tmpDir := t.TempDir()
	logPath := filepath.Join(tmpDir, "container.log")
	expired := logPath + ".2"
	originalPath := filepath.Join(tmpDir, "original-expired.log")
	if err := os.WriteFile(expired, []byte("original-expired"), 0644); err != nil {
		t.Fatal(err)
	}

	oldLstat := archiveLstat
	calls := 0
	archiveLstat = func(path string) (os.FileInfo, error) {
		fi, err := os.Lstat(path)
		if path == expired && err == nil {
			calls++
			if calls == 2 {
				if err := os.Rename(expired, originalPath); err != nil {
					t.Fatalf("move revalidated expired archive: %v", err)
				}
				if err := os.WriteFile(expired, []byte("replacement"), 0644); err != nil {
					t.Fatalf("replace expired archive after revalidation: %v", err)
				}
			}
		}
		return fi, err
	}
	defer func() { archiveLstat = oldLstat }()

	if err := ArchiveLogFile(logPath, 3); err == nil {
		t.Fatal("expected archive failure after expired archive replacement")
	}

	got, err := os.ReadFile(expired)
	if err != nil {
		t.Fatalf("replacement path should remain untouched: %v", err)
	}
	if string(got) != "replacement" {
		t.Fatalf("replacement path changed: %q", got)
	}
	got, err = os.ReadFile(originalPath)
	if err != nil {
		t.Fatalf("original inspected inode should remain available: %v", err)
	}
	if string(got) != "original-expired" {
		t.Fatalf("original inspected inode changed: %q", got)
	}
}
