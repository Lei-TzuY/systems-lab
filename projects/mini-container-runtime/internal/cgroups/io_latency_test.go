package cgroups

import (
	"testing"
)

func TestApplyIOLatency(t *testing.T) {
	tmpDir := t.TempDir()
	if err := ApplyIOLatency(tmpDir, 20); err != nil {
		t.Fatalf("ApplyIOLatency error: %v", err)
	}
}
