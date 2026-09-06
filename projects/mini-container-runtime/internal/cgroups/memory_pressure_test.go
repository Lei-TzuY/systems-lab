package cgroups

import (
	"os"
	"path/filepath"
	"runtime"
	"testing"
)

func TestReadMemoryPressureStallTotal_Missing(t *testing.T) {
	tmpDir := t.TempDir()
	total, err := ReadMemoryPressureStallTotal(tmpDir)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if total != 0 {
		t.Errorf("expected 0 for missing file, got %d", total)
	}
}

func TestReadMemoryPressureStallTotal_Success(t *testing.T) {
	if runtime.GOOS != "linux" {
		t.Skip("cgroup file reading only works on Linux")
	}

	tmpDir := t.TempDir()
	content := "some avg10=0.00 avg60=0.00 avg300=0.00 total=987654\nfull avg10=0.00 avg60=0.00 avg300=0.00 total=123\n"
	if err := os.WriteFile(filepath.Join(tmpDir, "memory.pressure"), []byte(content), 0644); err != nil {
		t.Fatal(err)
	}

	total, err := ReadMemoryPressureStallTotal(tmpDir)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if total != 987654 {
		t.Errorf("expected 987654, got %d", total)
	}
}
