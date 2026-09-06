package cgroups

import (
	"os"
	"path/filepath"
	"runtime"
	"testing"
)

func TestReadMemoryEventsLocalZswapMaxCount_Missing(t *testing.T) {
	tmpDir := t.TempDir()
	count, err := ReadMemoryEventsLocalZswapMaxCount(tmpDir)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if count != 0 {
		t.Errorf("expected 0 for missing file, got %d", count)
	}
}

func TestReadMemoryEventsLocalZswapMaxCount_Success(t *testing.T) {
	if runtime.GOOS != "linux" {
		t.Skip("cgroup file reading only works on Linux")
	}

	tmpDir := t.TempDir()
	content := "low 0\nhigh 0\nmax 0\nzswap_max 88\n"
	if err := os.WriteFile(filepath.Join(tmpDir, "memory.events.local"), []byte(content), 0644); err != nil {
		t.Fatal(err)
	}

	count, err := ReadMemoryEventsLocalZswapMaxCount(tmpDir)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if count != 88 {
		t.Errorf("expected 88, got %d", count)
	}
}
