//go:build linux

package state

import (
	"os"
	"strings"
	"testing"
)

func TestReadRegularStateFileRejectsHardLinkedInode(t *testing.T) {
	dir := t.TempDir()
	path := dir + "/state.json"
	alias := dir + "/alias.json"
	if err := os.WriteFile(path, []byte(`{"id":"abc"}`), 0o600); err != nil {
		t.Fatalf("WriteFile: %v", err)
	}
	if err := os.Link(path, alias); err != nil {
		t.Fatalf("Link: %v", err)
	}

	_, err := readRegularStateFile(path, "container state")
	if err == nil || !strings.Contains(err.Error(), "single-linked") {
		t.Fatalf("expected hard-link rejection, got %v", err)
	}
}
