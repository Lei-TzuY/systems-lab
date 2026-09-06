package cgroups

import (
	"os"
	"path/filepath"
	"runtime"
	"testing"
)

func TestReadMemoryEventsZswapMaxCount_Missing(t *testing.T) {
	tmpDir := t.TempDir()
	count, err := ReadMemoryEventsZswapMaxCount(tmpDir)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if count != 0 {
		t.Errorf("expected 0 for missing file, got %d", count)
	}
}

func TestReadMemoryEventsZswapMaxCount_Success(t *testing.T) {
	if runtime.GOOS != "linux" {
		t.Skip("cgroup file reading only works on Linux")
	}

	tmpDir := t.TempDir()
	content := "low 0\nhigh 0\nmax 0\noom 0\noom_kill 0\nzswap_max 42\n"
	if err := os.WriteFile(filepath.Join(tmpDir, "memory.events"), []byte(content), 0644); err != nil {
		t.Fatal(err)
	}

	count, err := ReadMemoryEventsZswapMaxCount(tmpDir)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if count != 42 {
		t.Errorf("expected 42, got %d", count)
	}
}
