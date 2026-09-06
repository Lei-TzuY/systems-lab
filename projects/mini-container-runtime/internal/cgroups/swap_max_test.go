package cgroups

import (
	"testing"
)

func TestApplySwapMax(t *testing.T) {
	tmpDir := t.TempDir()
	if err := ApplySwapMax(tmpDir, 52428800); err != nil {
		t.Fatalf("ApplySwapMax error: %v", err)
	}
}
