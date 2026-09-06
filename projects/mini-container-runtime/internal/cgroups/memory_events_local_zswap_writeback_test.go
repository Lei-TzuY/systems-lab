package cgroups

import (
	"os"
	"path/filepath"
	"runtime"
	"testing"
)

func TestReadMemoryEventsLocalZswapWritebackCount_Missing(t *testing.T) {
	tmpDir := t.TempDir()
	count, err := ReadMemoryEventsLocalZswapWritebackCount(tmpDir)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if count != 0 {
		t.Errorf("expected 0 for missing file, got %d", count)
	}
}

func TestReadMemoryEventsLocalZswapWritebackCount_Success(t *testing.T) {
	if runtime.GOOS != "linux" {
		t.Skip("cgroup file reading only works on Linux")
	}

	tmpDir := t.TempDir()
	content := "low 0\nhigh 0\nmax 0\noom 0\nzswap_writeback 77\n"
	if err := os.WriteFile(filepath.Join(tmpDir, "memory.events.local"), []byte(content), 0644); err != nil {
		t.Fatal(err)
	}

	count, err := ReadMemoryEventsLocalZswapWritebackCount(tmpDir)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if count != 77 {
		t.Errorf("expected 77, got %d", count)
	}
}
