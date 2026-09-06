package cgroups

import (
	"testing"
)

func TestReadLocalOOMKillCount(t *testing.T) {
	tmpDir := t.TempDir()
	count, err := ReadLocalOOMKillCount(tmpDir)
	if err != nil && count != 0 {
		t.Fatalf("ReadLocalOOMKillCount error: %v", err)
	}
}
