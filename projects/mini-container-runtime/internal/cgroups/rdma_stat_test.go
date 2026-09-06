package cgroups

import (
	"os"
	"path/filepath"
	"runtime"
	"testing"
)

func TestReadRDMACurrent_Missing(t *testing.T) {
	tmpDir := t.TempDir()
	stats, err := ReadRDMACurrent(tmpDir)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if len(stats) != 0 {
		t.Errorf("expected empty stats for missing file, got %+v", stats)
	}
}

func TestReadRDMACurrent_Success(t *testing.T) {
	if runtime.GOOS != "linux" {
		t.Skip("cgroup file reading only works on Linux")
	}

	tmpDir := t.TempDir()
	content := "mlx5_0 hca_handle=2 hca_object=150\nmlx5_1 hca_handle=1 hca_object=50\n"
	if err := os.WriteFile(filepath.Join(tmpDir, "rdma.current"), []byte(content), 0644); err != nil {
		t.Fatal(err)
	}

	stats, err := ReadRDMACurrent(tmpDir)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if len(stats) != 2 {
		t.Fatalf("len(stats) = %d, want 2", len(stats))
	}
	if stats[0].DeviceName != "mlx5_0" || stats[0].HCAHandle != 2 || stats[0].HCAObject != 150 {
		t.Errorf("unexpected stat 0: %+v", stats[0])
	}
}
