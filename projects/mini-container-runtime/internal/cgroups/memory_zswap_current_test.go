package cgroups

import (
	"os"
	"path/filepath"
	"runtime"
	"testing"
)

func TestReadMemoryZswapCurrent_Missing(t *testing.T) {
	tmpDir := t.TempDir()
	bytes, err := ReadMemoryZswapCurrent(tmpDir)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if bytes != 0 {
		t.Errorf("expected 0 for missing file, got %d", bytes)
	}
}

func TestReadMemoryZswapCurrent_Success(t *testing.T) {
	if runtime.GOOS != "linux" {
		t.Skip("cgroup file reading only works on Linux")
	}

	tmpDir := t.TempDir()
	if err := os.WriteFile(filepath.Join(tmpDir, "memory.zswap.current"), []byte("1048576\n"), 0644); err != nil {
		t.Fatal(err)
	}

	bytes, err := ReadMemoryZswapCurrent(tmpDir)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if bytes != 1048576 {
		t.Errorf("expected 1048576, got %d", bytes)
	}
}
