package state

import (
	"errors"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestValidateStateFileWriteAcceptsExactLimit(t *testing.T) {
	data := make([]byte, maxStateFileBytes)
	if err := validateStateFileWrite(data, "container state"); err != nil {
		t.Fatalf("validateStateFileWrite(exact limit): %v", err)
	}

	data = append(data, 0)
	if err := validateStateFileWrite(data, "container state"); err == nil || !strings.Contains(err.Error(), "size limit") {
		t.Fatalf("expected size-limit error, got %v", err)
	}
}

func TestSaveRejectsOversizedSerializedStateBeforePublication(t *testing.T) {
	store, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	defer store.Close()

	// Backslashes expand during JSON encoding. Keep the caller-owned string well
	// below 4 MiB while making the serialized authoritative record exceed it.
	c := &Container{
		ID:  "oversized-encoded-state",
		Env: []string{strings.Repeat("\\", int(maxStateFileBytes/2)+1)},
	}
	if err := store.Save(c); err == nil || !strings.Contains(err.Error(), "size limit") {
		t.Fatalf("expected serialized-size rejection, got %v", err)
	}
	if c.Revision != 0 {
		t.Fatalf("failed initial Save changed caller revision to %d", c.Revision)
	}

	_, err = os.Stat(filepath.Join(store.ctrDir, c.ID+".json"))
	if !errors.Is(err, os.ErrNotExist) {
		t.Fatalf("oversized initial Save published state file: %v", err)
	}
}

func TestOversizedUpdatePreservesDurableStateAndRevision(t *testing.T) {
	store, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	defer store.Close()

	c := &Container{ID: "preserve-on-oversize", Env: []string{"ORIGINAL=1"}}
	if err := store.Save(c); err != nil {
		t.Fatalf("initial Save: %v", err)
	}
	if c.Revision != 1 {
		t.Fatalf("initial revision = %d, want 1", c.Revision)
	}

	c.Env = []string{strings.Repeat("x", int(maxStateFileBytes))}
	if err := store.Save(c); err == nil || !strings.Contains(err.Error(), "size limit") {
		t.Fatalf("expected oversized update rejection, got %v", err)
	}
	if c.Revision != 1 {
		t.Fatalf("failed update changed caller revision to %d", c.Revision)
	}

	persisted, err := store.Get(c.ID)
	if err != nil {
		t.Fatalf("Get after rejected update: %v", err)
	}
	if persisted.Revision != 1 {
		t.Fatalf("persisted revision = %d, want 1", persisted.Revision)
	}
	if len(persisted.Env) != 1 || persisted.Env[0] != "ORIGINAL=1" {
		t.Fatalf("rejected update changed durable Env: %#v", persisted.Env)
	}
}
