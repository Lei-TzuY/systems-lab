package cgroups

import (
	"os"
	"path/filepath"
	"runtime"
	"testing"
)

func TestReadCPUWeight_Missing(t *testing.T) {
	tmpDir := t.TempDir()
	weight, err := ReadCPUWeight(tmpDir)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if weight != 0 {
		t.Errorf("expected 0 for missing file, got %d", weight)
	}
}

func TestReadCPUWeight_Default(t *testing.T) {
	if runtime.GOOS != "linux" {
		t.Skip("cgroup file reading only works on Linux")
	}
	tmpDir := t.TempDir()
	if err := os.WriteFile(filepath.Join(tmpDir, "cpu.weight"), []byte("100\n"), 0644); err != nil {
		t.Fatal(err)
	}
	weight, err := ReadCPUWeight(tmpDir)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if weight != 100 {
		t.Errorf("expected 100, got %d", weight)
	}
}

func TestReadCPUWeight_Custom(t *testing.T) {
	if runtime.GOOS != "linux" {
		t.Skip("cgroup file reading only works on Linux")
	}
	tmpDir := t.TempDir()
	if err := os.WriteFile(filepath.Join(tmpDir, "cpu.weight"), []byte("5000\n"), 0644); err != nil {
		t.Fatal(err)
	}
	weight, err := ReadCPUWeight(tmpDir)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if weight != 5000 {
		t.Errorf("expected 5000, got %d", weight)
	}
}

func TestReadCPUWeight_Empty(t *testing.T) {
	tmpDir := t.TempDir()
	weight, err := ReadCPUWeight(tmpDir)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if weight != 0 {
		t.Errorf("expected 0 for empty/missing file, got %d", weight)
	}
}
