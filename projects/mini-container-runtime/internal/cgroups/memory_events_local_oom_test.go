package cgroups

import (
	"testing"
)

func TestReadLocalOOMCount(t *testing.T) {
	tmpDir := t.TempDir()
	count, err := ReadLocalOOMCount(tmpDir)
	if err != nil && count != 0 {
		t.Fatalf("ReadLocalOOMCount error: %v", err)
	}
}
