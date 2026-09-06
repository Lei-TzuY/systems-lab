package logs

import (
	"os"
	"path/filepath"
	"testing"
)

func TestArchiveLogFileRejectsHardLinkedActiveLog(t *testing.T) {
	tmpDir := t.TempDir()
	logPath := filepath.Join(tmpDir, "container.log")
	aliasPath := filepath.Join(tmpDir, "alias.log")
	if err := os.WriteFile(logPath, []byte("content"), 0644); err != nil {
		t.Fatal(err)
	}
	if err := os.Link(logPath, aliasPath); err != nil {
		t.Fatal(err)
	}

	if err := ArchiveLogFile(logPath, 3); err == nil {
		t.Fatal("expected archive failure for hard-linked active log")
	}

	for _, path := range []string{logPath, aliasPath} {
		got, err := os.ReadFile(path)
		if err != nil {
			t.Fatalf("hard-linked source %q should remain: %v", path, err)
		}
		if string(got) != "content" {
			t.Fatalf("hard-linked source %q changed: %q", path, got)
		}
	}
	if _, err := os.Lstat(logPath + ".1"); !os.IsNotExist(err) {
		t.Fatalf("archive destination unexpectedly created: %v", err)
	}
}
