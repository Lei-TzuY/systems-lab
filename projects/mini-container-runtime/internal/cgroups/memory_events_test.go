package cgroups

import (
	"os"
	"path/filepath"
	"testing"
)

func TestReadMemoryEvents(t *testing.T) {
	tmpDir := t.TempDir()
	fixture := "low 1\nhigh 2\nmax 3\noom 4\noom_kill 5\n"
	if err := os.WriteFile(filepath.Join(tmpDir, "memory.events"), []byte(fixture), 0o644); err != nil {
		t.Fatalf("write memory.events fixture: %v", err)
	}
	events, err := ReadMemoryEvents(tmpDir)
	if err != nil {
		t.Fatalf("ReadMemoryEvents error: %v", err)
	}
	if events["oom_kill"] != 5 {
		t.Fatalf("oom_kill = %d, want 5", events["oom_kill"])
	}
}
