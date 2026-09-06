package cgroups

import (
	"os"
	"path/filepath"
	"runtime"
	"testing"
)

func TestReadLocalSwapMaxCount_Missing(t *testing.T) {
	tmpDir := t.TempDir()
	count, err := ReadLocalSwapMaxCount(tmpDir)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if count != 0 {
		t.Errorf("expected 0 for missing file, got %d", count)
	}
}

func TestReadLocalSwapMaxCount_Success(t *testing.T) {
	if runtime.GOOS != "linux" {
		t.Skip("cgroup file reading only works on Linux")
	}

	tmpDir := t.TempDir()
	content := "low 0\nhigh 10\nmax 88\nfailcnt 2\n"
	if err := os.WriteFile(filepath.Join(tmpDir, "memory.swap.events.local"), []byte(content), 0644); err != nil {
		t.Fatal(err)
	}

	count, err := ReadLocalSwapMaxCount(tmpDir)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if count != 88 {
		t.Errorf("expected 88, got %d", count)
	}
}
