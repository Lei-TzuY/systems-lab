package cgroups

import (
	"testing"
)

func TestReadCPUWeightNice_Missing(t *testing.T) {
	tmpDir := t.TempDir()
	nice, err := ReadCPUWeightNice(tmpDir)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if nice != 0 {
		t.Errorf("expected 0 for missing file, got %d", nice)
	}
}

func TestWriteCPUWeightNice_InvalidRange(t *testing.T) {
	tmpDir := t.TempDir()
	if err := WriteCPUWeightNice(tmpDir, -21); err == nil {
		t.Error("expected error for nice -21, got nil")
	}
	if err := WriteCPUWeightNice(tmpDir, 20); err == nil {
		t.Error("expected error for nice 20, got nil")
	}
}

func TestWriteCPUWeightNice_ValidRange(t *testing.T) {
	tmpDir := t.TempDir()
	// On non-Linux this is a no-op stub; on Linux it would write the file
	if err := WriteCPUWeightNice(tmpDir, -10); err != nil {
		t.Errorf("unexpected error for nice -10: %v", err)
	}
	if err := WriteCPUWeightNice(tmpDir, 19); err != nil {
		t.Errorf("unexpected error for nice 19: %v", err)
	}
}
