package cgroups

import (
	"os"
	"path/filepath"
	"testing"
)

func TestPIDsLimit(t *testing.T) {
	tmpDir := t.TempDir()
	if err := ApplyPIDsLimit(tmpDir, 100); err != nil {
		t.Fatalf("ApplyPIDsLimit error: %v", err)
	}
	if err := os.WriteFile(filepath.Join(tmpDir, "pids.current"), []byte("7\n"), 0o644); err != nil {
		t.Fatalf("write pids.current fixture: %v", err)
	}

	cur, err := ReadPIDsCurrent(tmpDir)
	if err != nil {
		t.Fatalf("ReadPIDsCurrent error: %v", err)
	}
	if cur != 7 {
		t.Fatalf("Current PIDs = %d, want 7", cur)
	}
}
