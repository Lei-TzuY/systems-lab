//go:build linux

package state

import (
	"os"
	"path/filepath"
	"strings"
	"testing"

	"golang.org/x/sys/unix"
)

func TestInspectRegularStateFDAcceptsDetachedInode(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "state.json")
	if err := os.WriteFile(path, []byte(`{"id":"abc"}`), 0o600); err != nil {
		t.Fatalf("WriteFile: %v", err)
	}

	fd, err := unix.Open(path, unix.O_RDONLY|unix.O_CLOEXEC|unix.O_NOFOLLOW, 0)
	if err != nil {
		t.Fatalf("open state file: %v", err)
	}
	defer unix.Close(fd)

	// Atomic rename replacement has the same namespace effect on the old inode:
	// after the old pathname is removed, an already-open descriptor remains a
	// valid snapshot but reports Nlink == 0.
	if err := os.Remove(path); err != nil {
		t.Fatalf("remove opened state pathname: %v", err)
	}

	st, err := inspectRegularStateFD(fd, path, "container state")
	if err != nil {
		t.Fatalf("detached opened inode rejected: %v", err)
	}
	if st.Nlink != 0 {
		t.Fatalf("detached inode Nlink = %d, want 0", st.Nlink)
	}
}

func TestInspectRegularStateFDRejectsHardLinkAlias(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "state.json")
	alias := filepath.Join(dir, "alias.json")
	if err := os.WriteFile(path, []byte(`{"id":"abc"}`), 0o600); err != nil {
		t.Fatalf("WriteFile: %v", err)
	}
	if err := os.Link(path, alias); err != nil {
		t.Fatalf("Link: %v", err)
	}

	fd, err := unix.Open(path, unix.O_RDONLY|unix.O_CLOEXEC|unix.O_NOFOLLOW, 0)
	if err != nil {
		t.Fatalf("open hard-linked state file: %v", err)
	}
	defer unix.Close(fd)

	_, err = inspectRegularStateFD(fd, path, "container state")
	if err == nil || !strings.Contains(err.Error(), "hard-link aliases") {
		t.Fatalf("expected hard-link alias rejection, got %v", err)
	}
}
