package cgroups

import (
	"testing"
)

func TestResetMemoryPeak(t *testing.T) {
	tmpDir := t.TempDir()
	if err := ResetMemoryPeak(tmpDir); err != nil {
		t.Fatalf("ResetMemoryPeak error: %v", err)
	}
}
