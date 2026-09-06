package cgroups

import (
	"os"
	"path/filepath"
	"runtime"
	"testing"
)

func TestReadMiscCurrent_Missing(t *testing.T) {
	tmpDir := t.TempDir()
	stats, err := ReadMiscCurrent(tmpDir)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if len(stats) != 0 {
		t.Errorf("expected empty stats for missing file, got %+v", stats)
	}
}

func TestReadMiscCurrent_Success(t *testing.T) {
	if runtime.GOOS != "linux" {
		t.Skip("cgroup file reading only works on Linux")
	}

	tmpDir := t.TempDir()
	content := "sev 4\nsev_es 2\ntdx 0\n"
	if err := os.WriteFile(filepath.Join(tmpDir, "misc.current"), []byte(content), 0644); err != nil {
		t.Fatal(err)
	}

	stats, err := ReadMiscCurrent(tmpDir)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if len(stats) != 3 {
		t.Fatalf("len(stats) = %d, want 3", len(stats))
	}
	if stats[0].ResourceName != "sev" || stats[0].Usage != 4 {
		t.Errorf("unexpected stat 0: %+v", stats[0])
	}
	if stats[1].ResourceName != "sev_es" || stats[1].Usage != 2 {
		t.Errorf("unexpected stat 1: %+v", stats[1])
	}
}
