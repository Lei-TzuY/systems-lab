package cgroups

import (
	"testing"
)

func TestReadMemoryHighCounter(t *testing.T) {
	tmpDir := t.TempDir()
	count, err := ReadMemoryHighCounter(tmpDir)
	if err != nil && count != 0 {
		t.Fatalf("ReadMemoryHighCounter error: %v", err)
	}
}
