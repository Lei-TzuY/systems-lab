package cgroups

import (
	"testing"
)

func TestReadLocalMemoryHighCount(t *testing.T) {
	tmpDir := t.TempDir()
	count, err := ReadLocalMemoryHighCount(tmpDir)
	if err != nil && count != 0 {
		t.Fatalf("ReadLocalMemoryHighCount error: %v", err)
	}
}
