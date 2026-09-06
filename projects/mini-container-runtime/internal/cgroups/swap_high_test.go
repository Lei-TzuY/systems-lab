package cgroups

import (
	"testing"
)

func TestApplySwapHigh(t *testing.T) {
	tmpDir := t.TempDir()
	if err := ApplySwapHigh(tmpDir, 26214400); err != nil {
		t.Fatalf("ApplySwapHigh error: %v", err)
	}
}
