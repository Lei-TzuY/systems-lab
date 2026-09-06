package cgroups

import (
	"testing"
)

func TestIsMemoryPeakHigh(t *testing.T) {
	tmpDir := t.TempDir()
	isHigh, err := IsMemoryPeakHigh(tmpDir, 0.9)
	if err != nil || isHigh {
		t.Fatalf("IsMemoryPeakHigh = %v (err=%v), want false", isHigh, err)
	}
}
