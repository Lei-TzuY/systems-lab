package cgroups

import (
	"testing"
)

func TestApplyIOCost(t *testing.T) {
	tmpDir := t.TempDir()
	if err := ApplyIOCost(tmpDir, true); err != nil {
		t.Fatalf("ApplyIOCost error: %v", err)
	}
}
