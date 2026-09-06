package cgroups

import (
	"os"
	"path/filepath"
	"runtime"
	"testing"
)

func TestReadCPUIdle_Missing(t *testing.T) {
	tmpDir := t.TempDir()
	idle, err := ReadCPUIdle(tmpDir)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if idle != 0 {
		t.Errorf("expected 0 for missing file, got %d", idle)
	}
}

func TestReadCPUIdle_Linux(t *testing.T) {
	if runtime.GOOS != "linux" {
		t.Skip("cgroup file reading only works on Linux")
	}

	tmpDir := t.TempDir()
	if err := os.WriteFile(filepath.Join(tmpDir, "cpu.idle"), []byte("1\n"), 0644); err != nil {
		t.Fatal(err)
	}

	idle, err := ReadCPUIdle(tmpDir)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if idle != 1 {
		t.Errorf("expected 1, got %d", idle)
	}
}

func TestWriteCPUIdle_Validation(t *testing.T) {
	tmpDir := t.TempDir()

	if err := WriteCPUIdle(tmpDir, 2); err == nil {
		t.Error("expected error for idle=2, got nil")
	}
	if err := WriteCPUIdle(tmpDir, -1); err == nil {
		t.Error("expected error for idle=-1, got nil")
	}

	if err := WriteCPUIdle(tmpDir, 1); err != nil {
		t.Errorf("unexpected error for idle=1: %v", err)
	}
	if err := WriteCPUIdle(tmpDir, 0); err != nil {
		t.Errorf("unexpected error for idle=0: %v", err)
	}
}
