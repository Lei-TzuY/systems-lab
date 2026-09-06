package cgroups

import (
	"errors"
	"os"
	"path/filepath"
	"runtime"
	"testing"
)

func TestReadPIDSPeak_Missing(t *testing.T) {
	tmpDir := t.TempDir()
	peak, err := ReadPIDSPeak(tmpDir)
	if !errors.Is(err, ErrPIDSPeakUnavailable) {
		t.Fatalf("error = %v, want ErrPIDSPeakUnavailable", err)
	}
	if peak != 0 {
		t.Errorf("peak = %d, want 0 on unavailable telemetry", peak)
	}
}

func TestResetPIDSPeak_IsReadOnly(t *testing.T) {
	if err := ResetPIDSPeak(t.TempDir()); !errors.Is(err, ErrPIDSPeakReadOnly) {
		t.Fatalf("error = %v, want ErrPIDSPeakReadOnly", err)
	}
}

func TestReadPIDSPeak_Linux(t *testing.T) {
	if runtime.GOOS != "linux" {
		t.Skip("pids.peak parsing is Linux-specific")
	}

	tmpDir := t.TempDir()
	peakPath := filepath.Join(tmpDir, "pids.peak")
	if err := os.WriteFile(peakPath, []byte("256\n"), 0o444); err != nil {
		t.Fatal(err)
	}

	val, err := ReadPIDSPeak(tmpDir)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if val != 256 {
		t.Errorf("val = %d, want 256", val)
	}

	if err := ResetPIDSPeak(tmpDir); !errors.Is(err, ErrPIDSPeakReadOnly) {
		t.Fatalf("reset error = %v, want ErrPIDSPeakReadOnly", err)
	}
	data, err := os.ReadFile(peakPath)
	if err != nil {
		t.Fatal(err)
	}
	if string(data) != "256\n" {
		t.Fatalf("read-only reset mutated pids.peak to %q", string(data))
	}
}

func TestReadPIDSPeak_InvalidValue_Linux(t *testing.T) {
	if runtime.GOOS != "linux" {
		t.Skip("pids.peak parsing is Linux-specific")
	}

	tests := []struct {
		name  string
		value string
	}{
		{name: "empty", value: "\n"},
		{name: "non-numeric", value: "max\n"},
		{name: "negative", value: "-1\n"},
	}

	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			tmpDir := t.TempDir()
			if err := os.WriteFile(filepath.Join(tmpDir, "pids.peak"), []byte(tc.value), 0o444); err != nil {
				t.Fatal(err)
			}
			if _, err := ReadPIDSPeak(tmpDir); err == nil {
				t.Fatalf("expected parse error for %q", tc.value)
			}
		})
	}
}
