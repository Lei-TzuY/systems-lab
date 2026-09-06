package cgroups

import (
	"errors"
	"os"
	"path/filepath"
	"runtime"
	"testing"
)

func TestReadCPUMaxBurst_Missing(t *testing.T) {
	tmpDir := t.TempDir()
	burst, err := ReadCPUMaxBurst(tmpDir)
	if !errors.Is(err, ErrCPUBurstUnavailable) {
		t.Fatalf("error = %v, want ErrCPUBurstUnavailable", err)
	}
	if burst != 0 {
		t.Errorf("expected 0 on unavailable telemetry, got %d", burst)
	}
}

func TestWriteCPUMaxBurst_Missing(t *testing.T) {
	tmpDir := t.TempDir()
	err := WriteCPUMaxBurst(tmpDir, 1000)
	if !errors.Is(err, ErrCPUBurstUnavailable) {
		t.Fatalf("error = %v, want ErrCPUBurstUnavailable", err)
	}
}

func TestWriteAndReadCPUMaxBurst_Linux(t *testing.T) {
	if runtime.GOOS != "linux" {
		t.Skip("cgroup file reading/writing is Linux-specific")
	}

	tmpDir := t.TempDir()
	burstFile := filepath.Join(tmpDir, "cpu.max.burst")
	if err := os.WriteFile(burstFile, []byte("0\n"), 0644); err != nil {
		t.Fatal(err)
	}

	if err := WriteCPUMaxBurst(tmpDir, 50000); err != nil {
		t.Fatal(err)
	}

	val, err := ReadCPUMaxBurst(tmpDir)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if val != 50000 {
		t.Errorf("val = %d, want 50000", val)
	}

	data, err := os.ReadFile(burstFile)
	if err != nil {
		t.Fatal(err)
	}
	if string(data) != "50000\n" {
		t.Errorf("file data = %q, want '50000\\n'", string(data))
	}
}

func TestReadCPUMaxBurst_InvalidValues_Linux(t *testing.T) {
	if runtime.GOOS != "linux" {
		t.Skip("cgroup file reading is Linux-specific")
	}

	tests := []struct {
		name  string
		value string
	}{
		{name: "empty", value: "\n"},
		{name: "non-numeric", value: "invalid\n"},
		{name: "negative", value: "-500\n"},
	}

	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			tmpDir := t.TempDir()
			burstFile := filepath.Join(tmpDir, "cpu.max.burst")
			if err := os.WriteFile(burstFile, []byte(tc.value), 0644); err != nil {
				t.Fatal(err)
			}

			if _, err := ReadCPUMaxBurst(tmpDir); err == nil {
				t.Fatalf("expected error for invalid value %q", tc.value)
			}
		})
	}
}
