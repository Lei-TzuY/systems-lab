package cgroups

import (
	"testing"
)

func TestReadMemoryLowCounter(t *testing.T) {
	tmpDir := t.TempDir()
	count, err := ReadMemoryLowCounter(tmpDir)
	if err != nil && count != 0 {
		t.Fatalf("ReadMemoryLowCounter error: %v", err)
	}
}
