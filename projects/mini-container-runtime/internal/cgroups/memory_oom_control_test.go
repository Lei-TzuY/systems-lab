package cgroups

import (
	"testing"
)

func TestSetMemoryOOMGroup(t *testing.T) {
	tmpDir := t.TempDir()
	if err := SetMemoryOOMGroup(tmpDir, true); err != nil {
		t.Fatalf("SetMemoryOOMGroup error: %v", err)
	}
}
