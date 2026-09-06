package cgroups

import (
	"testing"
)

func TestMemoryAdvanced(t *testing.T) {
	tmpDir := t.TempDir()
	if err := ApplyMemoryAdvanced(tmpDir, 67108864, 134217728); err != nil {
		t.Fatalf("ApplyMemoryAdvanced error: %v", err)
	}
}
