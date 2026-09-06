package cgroups

import (
	"testing"
)

func TestReadOOMKillCount(t *testing.T) {
	tmpDir := t.TempDir()
	count, err := ReadOOMKillCount(tmpDir)
	if err != nil && count != 0 {
		t.Fatalf("ReadOOMKillCount error: %v", err)
	}
}
