package cgroups

import (
	"testing"
)

func TestSetMemoryLow(t *testing.T) {
	tmpDir := t.TempDir()
	if err := SetMemoryLow(tmpDir, 20971520); err != nil {
		t.Fatalf("SetMemoryLow error: %v", err)
	}
}
