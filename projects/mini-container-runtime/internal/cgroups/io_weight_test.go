package cgroups

import (
	"testing"
)

func TestApplyIOWeight(t *testing.T) {
	tmpDir := t.TempDir()
	if err := ApplyIOWeight(tmpDir, 300); err != nil {
		t.Fatalf("ApplyIOWeight error: %v", err)
	}
}
