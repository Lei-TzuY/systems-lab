package cgroups

import (
	"os"
	"path/filepath"
	"runtime"
	"testing"
)

func TestReadMemoryNUMAStat_Missing(t *testing.T) {
	tmpDir := t.TempDir()
	stat, err := ReadMemoryNUMAStat(tmpDir)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if len(stat.Anon) != 0 || len(stat.File) != 0 {
		t.Errorf("expected empty stats for missing file, got %+v", stat)
	}
}

func TestReadMemoryNUMAStat_Success(t *testing.T) {
	if runtime.GOOS != "linux" {
		t.Skip("cgroup file reading only works on Linux")
	}

	tmpDir := t.TempDir()
	content := "anon N0=1048576 N1=2097152\nfile N0=524288 N1=0\nshmem N0=65536 N1=65536\n"
	if err := os.WriteFile(filepath.Join(tmpDir, "memory.numa_stat"), []byte(content), 0644); err != nil {
		t.Fatal(err)
	}

	stat, err := ReadMemoryNUMAStat(tmpDir)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	if stat.Anon[0] != 1048576 || stat.Anon[1] != 2097152 {
		t.Errorf("unexpected Anon stats: %+v", stat.Anon)
	}
	if stat.File[0] != 524288 || stat.File[1] != 0 {
		t.Errorf("unexpected File stats: %+v", stat.File)
	}
	if stat.Kernel[0] != 65536 || stat.Kernel[1] != 65536 {
		t.Errorf("unexpected Kernel stats: %+v", stat.Kernel)
	}
}
