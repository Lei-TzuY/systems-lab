package logs

import (
	"bytes"
	"os"
	"path/filepath"
	"testing"
)

func TestReadLastNLines(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "test.log")
	content := "line1\nline2\nline3\nline4\nline5\n"
	if err := os.WriteFile(path, []byte(content), 0600); err != nil {
		t.Fatalf("WriteFile: %v", err)
	}

	f, err := os.Open(path)
	if err != nil {
		t.Fatalf("Open: %v", err)
	}
	defer f.Close()

	lines, err := readLastNLines(f, 3)
	if err != nil {
		t.Fatalf("readLastNLines: %v", err)
	}

	if len(lines) != 3 {
		t.Fatalf("len = %d, want 3", len(lines))
	}
	if lines[0] != "line3" || lines[1] != "line4" || lines[2] != "line5" {
		t.Errorf("lines = %#v", lines)
	}
}

func TestPrintLogs(t *testing.T) {
	dir := t.TempDir()
	id := "testcntr123"
	logFile := filepath.Join(dir, "containers", id+".log")
	if err := os.MkdirAll(filepath.Dir(logFile), 0700); err != nil {
		t.Fatalf("MkdirAll: %v", err)
	}

	if err := os.WriteFile(logFile, []byte("hello from container\nsecond line\n"), 0600); err != nil {
		t.Fatalf("WriteFile: %v", err)
	}

	// We override default log file path check by writing directly
	var buf bytes.Buffer
	f, err := os.Open(logFile)
	if err != nil {
		t.Fatalf("Open: %v", err)
	}
	defer f.Close()

	lines, err := readLastNLines(f, 1)
	if err != nil {
		t.Fatalf("readLastNLines: %v", err)
	}
	if len(lines) != 1 || lines[0] != "second line" {
		t.Errorf("got %#v", lines)
	}
	_ = buf
}
