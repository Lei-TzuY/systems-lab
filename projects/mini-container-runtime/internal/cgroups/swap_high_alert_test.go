package cgroups

import (
	"testing"
)

func TestIsSwapHighExceeded(t *testing.T) {
	tmpDir := t.TempDir()
	exceeded, err := IsSwapHighExceeded(tmpDir)
	if err != nil || exceeded {
		t.Fatalf("IsSwapHighExceeded = %v (err=%v), want false", exceeded, err)
	}
}
