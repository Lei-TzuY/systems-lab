package cgroups

import (
	"testing"
)

func TestIOLimits(t *testing.T) {
	limits := IOLimits{
		ReadBPS:   10485760,
		WriteBPS:  5242880,
		ReadIOPS:  100,
		WriteIOPS: 50,
		Device:    "8:0",
	}

	tmpDir := t.TempDir()
	if err := ApplyIOMax(tmpDir, limits); err != nil {
		t.Fatalf("ApplyIOMax error: %v", err)
	}
}
