package cgroups

import (
	"testing"
)

func TestCPUSet(t *testing.T) {
	tmpDir := t.TempDir()
	if err := ApplyCPUSet(tmpDir, "0-2", "0"); err != nil {
		t.Fatalf("ApplyCPUSet error: %v", err)
	}
}
