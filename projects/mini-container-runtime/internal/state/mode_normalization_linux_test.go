//go:build linux

package state

import (
	"os"
	"path/filepath"
	"testing"
)

func assertPrivatePlainFileMode(t *testing.T, path string) {
	t.Helper()
	info, err := os.Stat(path)
	if err != nil {
		t.Fatal(err)
	}
	if got := info.Mode().Perm(); got != 0o600 {
		t.Fatalf("%s permissions=%#o, want 0600", path, got)
	}
	if info.Mode()&(os.ModeSetuid|os.ModeSetgid|os.ModeSticky) != 0 {
		t.Fatalf("%s retained special mode bits: %v", path, info.Mode())
	}
}

func TestStateReadClearsSpecialModeBits(t *testing.T) {
	root := t.TempDir()
	store, err := Open(root)
	if err != nil {
		t.Fatal(err)
	}
	if err := store.Save(&Container{ID: "mode-test", Status: StatusStopped}); err != nil {
		t.Fatal(err)
	}
	path := filepath.Join(root, "containers", "mode-test.json")
	if err := os.Chmod(path, 0o600|os.ModeSetuid|os.ModeSetgid|os.ModeSticky); err != nil {
		t.Fatal(err)
	}
	if _, err := store.Get("mode-test"); err != nil {
		t.Fatalf("Get: %v", err)
	}
	assertPrivatePlainFileMode(t, path)
}

func TestStateLockOpenClearsSpecialModeBits(t *testing.T) {
	root := t.TempDir()
	if _, err := Open(root); err != nil {
		t.Fatal(err)
	}
	path := filepath.Join(root, ".state.lock")
	if err := os.Chmod(path, 0o600|os.ModeSetuid|os.ModeSetgid|os.ModeSticky); err != nil {
		t.Fatal(err)
	}
	if _, err := Open(root); err != nil {
		t.Fatalf("reopen state store: %v", err)
	}
	assertPrivatePlainFileMode(t, path)
}
