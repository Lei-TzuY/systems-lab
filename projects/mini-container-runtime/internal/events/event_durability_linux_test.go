//go:build linux

package events

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestSyncEventLogDirectoryAcceptsRealDirectory(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "events.log")
	if err := syncEventLogDirectory(path); err != nil {
		t.Fatalf("sync real event directory: %v", err)
	}
}

func TestSyncEventLogDirectoryRejectsSymlinkDirectory(t *testing.T) {
	realDir := t.TempDir()
	parent := t.TempDir()
	linkDir := filepath.Join(parent, "state")
	if err := os.Symlink(realDir, linkDir); err != nil {
		t.Fatal(err)
	}

	err := syncEventLogDirectory(filepath.Join(linkDir, "events.log"))
	if err == nil {
		t.Fatal("expected symlinked event directory to fail closed")
	}
	if !strings.Contains(err.Error(), "open event log directory for sync") {
		t.Fatalf("unexpected error: %v", err)
	}
}

func TestEventLogAppendKeepsPathIdentityAcrossDurabilityBarrier(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "events.log")
	f, err := openEventLogForAppend(path)
	if err != nil {
		t.Fatalf("open durable event writer: %v", err)
	}
	defer f.Close()

	held, err := f.Stat()
	if err != nil {
		t.Fatal(err)
	}
	current, err := os.Lstat(path)
	if err != nil {
		t.Fatal(err)
	}
	if !os.SameFile(held, current) {
		t.Fatal("active event pathname does not identify the held writer inode")
	}
}
