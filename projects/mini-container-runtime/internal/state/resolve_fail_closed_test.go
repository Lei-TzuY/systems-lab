package state

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestResolveFailsClosedWhenExactStatePathIsDirectory(t *testing.T) {
	dir := t.TempDir()
	store, err := Open(dir)
	if err != nil {
		t.Fatalf("Open: %v", err)
	}
	defer store.Close()

	const exactID = "abc123"
	const siblingID = "abc123ffff"
	if err := store.Save(&Container{ID: exactID, Status: StatusStopped}); err != nil {
		t.Fatalf("Save exact: %v", err)
	}
	if err := store.Save(&Container{ID: siblingID, Status: StatusStopped}); err != nil {
		t.Fatalf("Save sibling: %v", err)
	}

	exactPath := filepath.Join(dir, "containers", exactID+".json")
	if err := os.Remove(exactPath); err != nil {
		t.Fatalf("remove exact state file: %v", err)
	}
	if err := os.Mkdir(exactPath, 0o700); err != nil {
		t.Fatalf("replace exact state file with directory: %v", err)
	}

	got, err := store.Resolve(exactID)
	if err == nil {
		t.Fatalf("Resolve(%q) unexpectedly returned container %q", exactID, got.ID)
	}
	if !strings.Contains(err.Error(), "container state") {
		t.Fatalf("Resolve(%q) error = %v, want exact state read failure", exactID, err)
	}
}

func TestResolvePreservesExactCorruptionInsteadOfPrefixFallback(t *testing.T) {
	dir := t.TempDir()
	store, err := Open(dir)
	if err != nil {
		t.Fatalf("Open: %v", err)
	}
	defer store.Close()

	const exactID = "def456"
	const siblingID = "def456ffff"
	if err := store.Save(&Container{ID: exactID, Status: StatusStopped}); err != nil {
		t.Fatalf("Save exact: %v", err)
	}
	if err := store.Save(&Container{ID: siblingID, Status: StatusStopped}); err != nil {
		t.Fatalf("Save sibling: %v", err)
	}

	exactPath := filepath.Join(dir, "containers", exactID+".json")
	if err := os.WriteFile(exactPath, []byte("{not-json"), 0o600); err != nil {
		t.Fatalf("corrupt exact state: %v", err)
	}

	if _, err := store.Resolve(exactID); err == nil {
		t.Fatalf("Resolve(%q) unexpectedly succeeded", exactID)
	} else if !strings.Contains(err.Error(), "unmarshal container state") {
		t.Fatalf("Resolve(%q) error = %v, want exact unmarshal failure", exactID, err)
	}
}

func TestResolveStillSupportsMissingExactPrefix(t *testing.T) {
	dir := t.TempDir()
	store, err := Open(dir)
	if err != nil {
		t.Fatalf("Open: %v", err)
	}
	defer store.Close()

	const id = "feedface00112233"
	if err := store.Save(&Container{ID: id, Status: StatusStopped}); err != nil {
		t.Fatalf("Save: %v", err)
	}

	got, err := store.Resolve("feed")
	if err != nil {
		t.Fatalf("Resolve prefix: %v", err)
	}
	if got.ID != id {
		t.Fatalf("Resolve prefix ID = %q, want %q", got.ID, id)
	}
}
