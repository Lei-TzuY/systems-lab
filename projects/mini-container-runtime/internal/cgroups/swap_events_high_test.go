package cgroups

import (
	"testing"
)

func TestReadSwapHighCount(t *testing.T) {
	tmpDir := t.TempDir()
	count, err := ReadSwapHighCount(tmpDir)
	if err != nil && count != 0 {
		t.Fatalf("ReadSwapHighCount error: %v", err)
	}
}
