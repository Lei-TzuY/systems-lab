package cgroups

import (
	"os"
	"path/filepath"
	"runtime"
	"testing"
)

func TestReadMemoryStatBreakdown_Missing(t *testing.T) {
	tmpDir := t.TempDir()
	b, err := ReadMemoryStatBreakdown(tmpDir)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if b.Anon != 0 || b.UserTotal != 0 {
		t.Errorf("expected empty stats for missing file, got %+v", b)
	}
}

func TestReadMemoryStatBreakdown_Success(t *testing.T) {
	if runtime.GOOS != "linux" {
		t.Skip("cgroup file reading only works on Linux")
	}

	tmpDir := t.TempDir()
	content := "anon 1000000\nfile 2000000\nshmem 500000\nkernel_stack 100000\npagetables 50000\nslab 300000\nslab_reclaimable 200000\nslab_unreclaimable 100000\npgfault 5000\npgmajfault 50\n"
	if err := os.WriteFile(filepath.Join(tmpDir, "memory.stat"), []byte(content), 0644); err != nil {
		t.Fatal(err)
	}

	b, err := ReadMemoryStatBreakdown(tmpDir)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if b.Anon != 1000000 {
		t.Errorf("Anon = %d, want 1000000", b.Anon)
	}
	if b.UserTotal != 3500000 {
		t.Errorf("UserTotal = %d, want 3500000", b.UserTotal)
	}
	if b.SlabReclaimRatio < 0.66 || b.SlabReclaimRatio > 0.67 {
		t.Errorf("SlabReclaimRatio = %f, want ~0.666", b.SlabReclaimRatio)
	}
}
