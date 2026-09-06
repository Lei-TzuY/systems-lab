package cgroups

import (
	"testing"
)

func TestReclaimMemory(t *testing.T) {
	tmpDir := t.TempDir()
	if err := ReclaimMemory(tmpDir, 4096); err != nil {
		t.Fatalf("ReclaimMemory error: %v", err)
	}
}
