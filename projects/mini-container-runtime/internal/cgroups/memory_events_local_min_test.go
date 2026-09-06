package cgroups

import (
	"testing"
)

func TestReadLocalMemoryMinCount(t *testing.T) {
	tmpDir := t.TempDir()
	count, err := ReadLocalMemoryMinCount(tmpDir)
	if err != nil && count != 0 {
		t.Fatalf("ReadLocalMemoryMinCount error: %v", err)
	}
}
