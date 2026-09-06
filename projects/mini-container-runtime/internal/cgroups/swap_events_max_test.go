package cgroups

import (
	"testing"
)

func TestReadSwapMaxCount(t *testing.T) {
	tmpDir := t.TempDir()
	count, err := ReadSwapMaxCount(tmpDir)
	if err != nil && count != 0 {
		t.Fatalf("ReadSwapMaxCount error: %v", err)
	}
}
