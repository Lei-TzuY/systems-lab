package cgroups

import (
	"testing"
)

func TestApplyMemoryHigh(t *testing.T) {
	tmpDir := t.TempDir()
	if err := ApplyMemoryHigh(tmpDir, 104857600); err != nil {
		t.Fatalf("ApplyMemoryHigh error: %v", err)
	}
}
