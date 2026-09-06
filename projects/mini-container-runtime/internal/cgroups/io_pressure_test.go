package cgroups

import (
	"os"
	"path/filepath"
	"runtime"
	"testing"
)

func TestReadIOPressureStallTotal_Missing(t *testing.T) {
	tmpDir := t.TempDir()
	total, err := ReadIOPressureStallTotal(tmpDir)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if total != 0 {
		t.Errorf("expected 0 for missing file, got %d", total)
	}
}

func TestReadIOPressureStallTotal_Success(t *testing.T) {
	if runtime.GOOS != "linux" {
		t.Skip("cgroup file reading only works on Linux")
	}

	tmpDir := t.TempDir()
	content := "some avg10=1.20 avg60=0.80 avg300=0.30 total=112233\nfull avg10=0.50 avg60=0.20 avg300=0.05 total=44556\n"
	if err := os.WriteFile(filepath.Join(tmpDir, "io.pressure"), []byte(content), 0644); err != nil {
		t.Fatal(err)
	}

	total, err := ReadIOPressureStallTotal(tmpDir)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if total != 112233 {
		t.Errorf("expected 112233, got %d", total)
	}
}
