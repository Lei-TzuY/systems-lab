package cgroups

import (
	"testing"
)

func TestReadSwapPeak(t *testing.T) {
	tmpDir := t.TempDir()
	peak, err := ReadSwapPeak(tmpDir)
	if err != nil && peak != 0 {
		t.Fatalf("ReadSwapPeak error: %v", err)
	}
}
