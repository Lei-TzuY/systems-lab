package cgroups

import (
	"os"
	"path/filepath"
	"testing"
)

func TestReadOOMEvents(t *testing.T) {
	tmpDir := t.TempDir()
	fixture := "low 0\nhigh 0\nmax 0\noom 2\noom_kill 1\n"
	if err := os.WriteFile(filepath.Join(tmpDir, "memory.events"), []byte(fixture), 0o644); err != nil {
		t.Fatalf("write memory.events fixture: %v", err)
	}
	evts, err := ReadOOMEvents(tmpDir)
	if err != nil {
		t.Fatalf("ReadOOMEvents error: %v", err)
	}
	if evts == nil {
		t.Fatalf("ReadOOMEvents returned nil struct")
	}
}
