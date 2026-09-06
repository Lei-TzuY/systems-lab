package cgroups

import (
	"testing"
)

func TestSetMemoryMin(t *testing.T) {
	tmpDir := t.TempDir()
	if err := SetMemoryMin(tmpDir, 10485760); err != nil {
		t.Fatalf("SetMemoryMin error: %v", err)
	}
}
