package cgroups

import (
	"testing"
)

func TestReadMemoryMinCounter(t *testing.T) {
	tmpDir := t.TempDir()
	count, err := ReadMemoryMinCounter(tmpDir)
	if err != nil && count != 0 {
		t.Fatalf("ReadMemoryMinCounter error: %v", err)
	}
}
