package cgroups

import (
	"os"
	"path/filepath"
	"runtime"
	"testing"
)

func TestReadLocalSwapHighCount_Missing(t *testing.T) {
	tmpDir := t.TempDir()
	count, err := ReadLocalSwapHighCount(tmpDir)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if count != 0 {
		t.Errorf("expected 0 for missing file, got %d", count)
	}
}

func TestReadLocalSwapHighCount_Success(t *testing.T) {
	if runtime.GOOS != "linux" {
		t.Skip("cgroup file reading only works on Linux")
	}

	tmpDir := t.TempDir()
	content := "low 0\nhigh 42\nmax 5\nfailcnt 0\n"
	if err := os.WriteFile(filepath.Join(tmpDir, "memory.swap.events.local"), []byte(content), 0644); err != nil {
		t.Fatal(err)
	}

	count, err := ReadLocalSwapHighCount(tmpDir)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if count != 42 {
		t.Errorf("expected 42, got %d", count)
	}
}
