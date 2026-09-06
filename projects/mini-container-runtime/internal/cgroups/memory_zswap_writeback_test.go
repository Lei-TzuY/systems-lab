package cgroups

import (
	"os"
	"path/filepath"
	"runtime"
	"testing"
)

func TestReadMemoryZswapWriteback_Missing(t *testing.T) {
	tmpDir := t.TempDir()
	val, err := ReadMemoryZswapWriteback(tmpDir)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if val != 1 {
		t.Errorf("expected 1 (default enabled) for missing file, got %d", val)
	}
}

func TestReadMemoryZswapWriteback_Linux(t *testing.T) {
	if runtime.GOOS != "linux" {
		t.Skip("cgroup file reading only works on Linux")
	}

	tmpDir := t.TempDir()
	if err := os.WriteFile(filepath.Join(tmpDir, "memory.zswap.writeback"), []byte("0\n"), 0644); err != nil {
		t.Fatal(err)
	}

	val, err := ReadMemoryZswapWriteback(tmpDir)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if val != 0 {
		t.Errorf("expected 0, got %d", val)
	}
}

func TestWriteMemoryZswapWriteback_Validation(t *testing.T) {
	tmpDir := t.TempDir()

	if err := WriteMemoryZswapWriteback(tmpDir, 2); err == nil {
		t.Error("expected error for enabled=2")
	}
	if err := WriteMemoryZswapWriteback(tmpDir, -1); err == nil {
		t.Error("expected error for enabled=-1")
	}

	if err := WriteMemoryZswapWriteback(tmpDir, 0); err != nil {
		t.Errorf("unexpected error for enabled=0: %v", err)
	}
	if err := WriteMemoryZswapWriteback(tmpDir, 1); err != nil {
		t.Errorf("unexpected error for enabled=1: %v", err)
	}
}
