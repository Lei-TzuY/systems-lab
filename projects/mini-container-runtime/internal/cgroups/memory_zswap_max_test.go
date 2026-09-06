package cgroups

import (
	"os"
	"path/filepath"
	"runtime"
	"testing"
)

func TestReadMemoryZswapMax_Missing(t *testing.T) {
	tmpDir := t.TempDir()
	val, err := ReadMemoryZswapMax(tmpDir)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if val != "max" {
		t.Errorf("expected max for missing file, got %q", val)
	}
}

func TestReadMemoryZswapMax_Linux(t *testing.T) {
	if runtime.GOOS != "linux" {
		t.Skip("cgroup file reading only works on Linux")
	}

	tmpDir := t.TempDir()
	if err := os.WriteFile(filepath.Join(tmpDir, "memory.zswap.max"), []byte("209715200\n"), 0644); err != nil {
		t.Fatal(err)
	}

	val, err := ReadMemoryZswapMax(tmpDir)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if val != "209715200" {
		t.Errorf("expected 209715200, got %q", val)
	}
}

func TestWriteMemoryZswapMax_Validation(t *testing.T) {
	tmpDir := t.TempDir()

	if err := WriteMemoryZswapMax(tmpDir, ""); err == nil {
		t.Error("expected error for empty limit")
	}
	if err := WriteMemoryZswapMax(tmpDir, "invalid_num"); err == nil {
		t.Error("expected error for invalid limit")
	}

	if err := WriteMemoryZswapMax(tmpDir, "max"); err != nil {
		t.Errorf("unexpected error for max: %v", err)
	}
	if err := WriteMemoryZswapMax(tmpDir, "104857600"); err != nil {
		t.Errorf("unexpected error for numeric limit: %v", err)
	}
}
