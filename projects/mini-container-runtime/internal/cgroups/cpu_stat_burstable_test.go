package cgroups

import (
	"os"
	"path/filepath"
	"runtime"
	"testing"
)

func TestReadCPUBurstThrottledMetrics_Missing(t *testing.T) {
	tmpDir := t.TempDir()
	m, err := ReadCPUBurstThrottledMetrics(tmpDir)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if m.UsageUsec != 0 || m.ThrottleRatio != 0.0 {
		t.Errorf("expected 0 for missing file, got %+v", m)
	}
}

func TestReadCPUBurstThrottledMetrics_Success(t *testing.T) {
	if runtime.GOOS != "linux" {
		t.Skip("cgroup file reading only works on Linux")
	}

	tmpDir := t.TempDir()
	content := "usage_usec 1000000\nuser_usec 800000\nsystem_usec 200000\nnr_periods 1000\nnr_throttled 200\nthrottled_usec 150000\nnr_bursts 50\nburst_usec 100000\n"
	if err := os.WriteFile(filepath.Join(tmpDir, "cpu.stat"), []byte(content), 0644); err != nil {
		t.Fatal(err)
	}

	m, err := ReadCPUBurstThrottledMetrics(tmpDir)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if m.UsageUsec != 1000000 {
		t.Errorf("UsageUsec = %d, want 1000000", m.UsageUsec)
	}
	if m.ThrottleRatio != 0.2 {
		t.Errorf("ThrottleRatio = %f, want 0.2", m.ThrottleRatio)
	}
	if m.BurstRatio != 0.1 {
		t.Errorf("BurstRatio = %f, want 0.1", m.BurstRatio)
	}
}
