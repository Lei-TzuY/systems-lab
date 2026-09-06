package cgroups

import (
	"os"
	"path/filepath"
	"testing"
)

func TestReadSwapEvents(t *testing.T) {
	tmpDir := t.TempDir()
	fixture := "high 1\nmax 2\nfail 3\n"
	if err := os.WriteFile(filepath.Join(tmpDir, "memory.swap.events"), []byte(fixture), 0o644); err != nil {
		t.Fatalf("write memory.swap.events fixture: %v", err)
	}
	events, err := ReadSwapEvents(tmpDir)
	if err != nil {
		t.Fatalf("ReadSwapEvents error: %v", err)
	}
	if events["fail"] != 3 {
		t.Fatalf("fail = %d, want 3", events["fail"])
	}
}
