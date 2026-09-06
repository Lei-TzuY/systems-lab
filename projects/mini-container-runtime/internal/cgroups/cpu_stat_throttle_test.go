package cgroups

import (
	"os"
	"path/filepath"
	"runtime"
	"testing"
)

func TestReadCPUStatThrottleMetrics_Missing(t *testing.T) {
	tmpDir := t.TempDir()
	m, err := ReadCPUStatThrottleMetrics(tmpDir)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if m.NrPeriods != 0 {
		t.Errorf("expected 0 for missing file, got %d", m.NrPeriods)
	}
}

func TestReadCPUStatThrottleMetrics_Success(t *testing.T) {
	if runtime.GOOS != "linux" {
		t.Skip("cgroup file reading only works on Linux")
	}

	tmpDir := t.TempDir()
	content := "usage_usec 123456\nuser_usec 100000\nsystem_usec 23456\nnr_periods 500\nnr_throttled 50\nthrottled_usec 12345\nnr_bursts 10\nburst_usec 999\n"
	if err := os.WriteFile(filepath.Join(tmpDir, "cpu.stat"), []byte(content), 0644); err != nil {
		t.Fatal(err)
	}

	m, err := ReadCPUStatThrottleMetrics(tmpDir)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if m.NrPeriods != 500 {
		t.Errorf("NrPeriods = %d, want 500", m.NrPeriods)
	}
	if m.NrThrottled != 50 {
		t.Errorf("NrThrottled = %d, want 50", m.NrThrottled)
	}
	if m.ThrottledUsec != 12345 {
		t.Errorf("ThrottledUsec = %d, want 12345", m.ThrottledUsec)
	}
	pct := m.ThrottlePercent()
	if pct < 9.9 || pct > 10.1 {
		t.Errorf("ThrottlePercent() = %f, want ~10.0", pct)
	}
}

func TestCPUStatThrottleMetrics_ThrottlePercentZero(t *testing.T) {
	m := CPUStatThrottleMetrics{}
	if m.ThrottlePercent() != 0 {
		t.Errorf("ThrottlePercent() = %f, want 0 for zero periods", m.ThrottlePercent())
	}
}
