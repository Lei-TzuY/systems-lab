package cgroups

import (
	"os"
	"path/filepath"
	"testing"
)

func TestReadMemoryEventsLocal(t *testing.T) {
	tmpDir := t.TempDir()
	fixture := "low 1\nhigh 2\nmax 3\noom 4\noom_kill 5\n"
	if err := os.WriteFile(filepath.Join(tmpDir, "memory.events.local"), []byte(fixture), 0o644); err != nil {
		t.Fatalf("write memory.events.local fixture: %v", err)
	}
	events, err := ReadMemoryEventsLocal(tmpDir)
	if err != nil {
		t.Fatalf("ReadMemoryEventsLocal error: %v", err)
	}
	if events["high"] != 2 {
		t.Fatalf("high = %d, want 2", events["high"])
	}
}
