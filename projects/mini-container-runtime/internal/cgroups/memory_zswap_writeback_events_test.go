package cgroups

import (
	"os"
	"path/filepath"
	"runtime"
	"testing"
)

func TestReadMemoryEventsZswapWritebackCount_Missing(t *testing.T) {
	tmpDir := t.TempDir()
	count, err := ReadMemoryEventsZswapWritebackCount(tmpDir)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if count != 0 {
		t.Errorf("expected 0 for missing file, got %d", count)
	}
}

func TestReadMemoryEventsZswapWritebackCount_Success(t *testing.T) {
	if runtime.GOOS != "linux" {
		t.Skip("cgroup file reading only works on Linux")
	}

	tmpDir := t.TempDir()
	content := "low 0\nhigh 0\nmax 0\noom 0\noom_kill 0\nzswap_writeback 128\n"
	if err := os.WriteFile(filepath.Join(tmpDir, "memory.events"), []byte(content), 0644); err != nil {
		t.Fatal(err)
	}

	count, err := ReadMemoryEventsZswapWritebackCount(tmpDir)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if count != 128 {
		t.Errorf("expected 128, got %d", count)
	}
}
