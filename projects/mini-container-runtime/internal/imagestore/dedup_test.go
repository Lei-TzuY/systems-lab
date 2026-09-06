package imagestore

import (
	"testing"

	"minicontainer/internal/state"
)

func TestDeduplicateImages(t *testing.T) {
	tmpDir := t.TempDir()
	st, err := state.Open(tmpDir)
	if err != nil {
		t.Fatalf("Open state store error: %v", err)
	}

	saved, err := DeduplicateImages(st)
	if err != nil {
		t.Fatalf("DeduplicateImages error: %v", err)
	}
	if saved < 0 {
		t.Fatalf("Saved bytes = %d, want >= 0", saved)
	}
}
