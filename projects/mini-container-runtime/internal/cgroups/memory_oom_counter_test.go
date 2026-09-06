package cgroups

import (
	"testing"
)

func TestReadOOMCounter(t *testing.T) {
	tmpDir := t.TempDir()
	count, err := ReadOOMCounter(tmpDir)
	if err != nil && count != 0 {
		t.Fatalf("ReadOOMCounter error: %v", err)
	}
}
