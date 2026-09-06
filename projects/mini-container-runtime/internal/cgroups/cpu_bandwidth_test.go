package cgroups

import (
	"testing"
)

func TestCPUBandwidthLimits(t *testing.T) {
	tmpDir := t.TempDir()
	if err := ApplyCPUWeight(tmpDir, 200); err != nil {
		t.Fatalf("ApplyCPUWeight error: %v", err)
	}

	if err := ApplyCPUMax(tmpDir, 50000, 100000); err != nil {
		t.Fatalf("ApplyCPUMax error: %v", err)
	}
}
