package cgroups

import (
	"testing"
)

func TestHugePagesLimit(t *testing.T) {
	tmpDir := t.TempDir()
	if err := ApplyHugeTLBLimit(tmpDir, "2MB", 104857600); err != nil {
		t.Fatalf("ApplyHugeTLBLimit error: %v", err)
	}
}
