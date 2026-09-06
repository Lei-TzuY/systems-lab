package cgroups

import (
	"testing"
)

func TestReadSwapFailcntCount(t *testing.T) {
	tmpDir := t.TempDir()
	count, err := ReadSwapFailcntCount(tmpDir)
	if err != nil && count != 0 {
		t.Fatalf("ReadSwapFailcntCount error: %v", err)
	}
}
