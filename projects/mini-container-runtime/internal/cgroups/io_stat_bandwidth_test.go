package cgroups

import (
	"os"
	"path/filepath"
	"runtime"
	"testing"
)

func TestReadIOStatSummary_Missing(t *testing.T) {
	tmpDir := t.TempDir()
	summary, err := ReadIOStatSummary(tmpDir)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if summary.TotalRBytes != 0 || len(summary.Devices) != 0 {
		t.Errorf("expected empty stats for missing file, got %+v", summary)
	}
}

func TestReadIOStatSummary_Success(t *testing.T) {
	if runtime.GOOS != "linux" {
		t.Skip("cgroup file reading only works on Linux")
	}

	tmpDir := t.TempDir()
	content := "8:0 rbytes=1048576 wbytes=2097152 rios=100 wios=200 dbytes=0 dios=0\n8:16 rbytes=524288 wbytes=0 rios=50 wios=0 dbytes=0 dios=0\n"
	if err := os.WriteFile(filepath.Join(tmpDir, "io.stat"), []byte(content), 0644); err != nil {
		t.Fatal(err)
	}

	summary, err := ReadIOStatSummary(tmpDir)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if summary.TotalRBytes != 1572864 {
		t.Errorf("TotalRBytes = %d, want 1572864", summary.TotalRBytes)
	}
	if summary.TotalWBytes != 2097152 {
		t.Errorf("TotalWBytes = %d, want 2097152", summary.TotalWBytes)
	}
	if summary.TotalRIOs != 150 {
		t.Errorf("TotalRIOs = %d, want 150", summary.TotalRIOs)
	}
	if summary.TotalWIOs != 200 {
		t.Errorf("TotalWIOs = %d, want 200", summary.TotalWIOs)
	}
	if len(summary.Devices) != 2 {
		t.Errorf("len(Devices) = %d, want 2", len(summary.Devices))
	}
}
