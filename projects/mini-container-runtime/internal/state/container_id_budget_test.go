package state

import (
	"os"
	"strings"
	"testing"
)

func TestValidateIDEnforcesFilenameBudget(t *testing.T) {
	atLimit := strings.Repeat("a", maxContainerIDBytes)
	if err := validateID(atLimit); err != nil {
		t.Fatalf("validateID(%d bytes): %v", len(atLimit), err)
	}

	overLimit := atLimit + "b"
	if err := validateID(overLimit); err == nil {
		t.Fatalf("validateID(%d bytes) unexpectedly succeeded", len(overLimit))
	}

	if got := len(atLimit + exitedIdentityRequiredSuffix); got > 255 {
		t.Fatalf("longest legacy sidecar component exceeds conservative 255-byte budget: %d", got)
	}
}

func TestOverlongContainerIDFailsBeforeFilesystemMutation(t *testing.T) {
	dir := t.TempDir()
	store, err := Open(dir)
	if err != nil {
		t.Fatal(err)
	}
	defer store.Close()

	id := strings.Repeat("a", maxContainerIDBytes+1)
	if err := store.Save(&Container{ID: id}); err == nil {
		t.Fatal("Save unexpectedly accepted overlong container ID")
	}
	if _, err := store.Get(id); err == nil {
		t.Fatal("Get unexpectedly accepted overlong container ID")
	}
	if _, err := store.Resolve(id); err == nil {
		t.Fatal("Resolve unexpectedly accepted overlong container ID")
	}
	if err := store.Delete(id); err == nil {
		t.Fatal("Delete unexpectedly accepted overlong container ID")
	}

	entries, err := os.ReadDir(store.ctrDir)
	if err != nil {
		t.Fatal(err)
	}
	if len(entries) != 0 {
		t.Fatalf("overlong ID caused filesystem mutation: %v", entries)
	}
}
