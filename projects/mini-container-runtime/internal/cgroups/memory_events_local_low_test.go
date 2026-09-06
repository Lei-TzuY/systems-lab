package cgroups

import (
	"testing"
)

func TestReadLocalMemoryLowCount(t *testing.T) {
	tmpDir := t.TempDir()
	count, err := ReadLocalMemoryLowCount(tmpDir)
	if err != nil && count != 0 {
		t.Fatalf("ReadLocalMemoryLowCount error: %v", err)
	}
}
