package cgroups

import (
	"os"
	"path/filepath"
	"runtime"
	"testing"
)

func TestReadPIDSEventsMaxCount_Missing(t *testing.T) {
	tmpDir := t.TempDir()
	count, err := ReadPIDSEventsMaxCount(tmpDir)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if count != 0 {
		t.Errorf("expected 0 for missing file, got %d", count)
	}
}

func TestReadPIDSEventsMaxCount_Success(t *testing.T) {
	if runtime.GOOS != "linux" {
		t.Skip("cgroup file reading only works on Linux")
	}

	tmpDir := t.TempDir()
	content := "max 17\n"
	if err := os.WriteFile(filepath.Join(tmpDir, "pids.events"), []byte(content), 0644); err != nil {
		t.Fatal(err)
	}

	count, err := ReadPIDSEventsMaxCount(tmpDir)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if count != 17 {
		t.Errorf("expected 17, got %d", count)
	}
}
