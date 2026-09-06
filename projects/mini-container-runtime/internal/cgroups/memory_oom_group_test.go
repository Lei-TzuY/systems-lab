package cgroups

import (
	"os"
	"path/filepath"
	"runtime"
	"testing"
)

func TestReadMemoryOOMGroup_Missing(t *testing.T) {
	tmpDir := t.TempDir()
	val, err := ReadMemoryOOMGroup(tmpDir)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if val != 0 {
		t.Errorf("expected 0 for missing file, got %d", val)
	}
}

func TestReadMemoryOOMGroup_Linux(t *testing.T) {
	if runtime.GOOS != "linux" {
		t.Skip("cgroup file reading only works on Linux")
	}

	tmpDir := t.TempDir()
	if err := os.WriteFile(filepath.Join(tmpDir, "memory.oom.group"), []byte("1\n"), 0644); err != nil {
		t.Fatal(err)
	}

	val, err := ReadMemoryOOMGroup(tmpDir)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if val != 1 {
		t.Errorf("expected 1, got %d", val)
	}
}

func TestWriteMemoryOOMGroup_Validation(t *testing.T) {
	tmpDir := t.TempDir()

	if err := WriteMemoryOOMGroup(tmpDir, 2); err == nil {
		t.Error("expected error for enabled=2")
	}
	if err := WriteMemoryOOMGroup(tmpDir, -1); err == nil {
		t.Error("expected error for enabled=-1")
	}

	if err := WriteMemoryOOMGroup(tmpDir, 1); err != nil {
		t.Errorf("unexpected error for enabled=1: %v", err)
	}
	if err := WriteMemoryOOMGroup(tmpDir, 0); err != nil {
		t.Errorf("unexpected error for enabled=0: %v", err)
	}
}
