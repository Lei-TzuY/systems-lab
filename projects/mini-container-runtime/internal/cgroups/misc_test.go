package cgroups

import (
	"testing"
)

func TestMiscLimit(t *testing.T) {
	tmpDir := t.TempDir()
	if err := ApplyMiscLimit(tmpDir, "sev", 10); err != nil {
		t.Fatalf("ApplyMiscLimit error: %v", err)
	}
}
