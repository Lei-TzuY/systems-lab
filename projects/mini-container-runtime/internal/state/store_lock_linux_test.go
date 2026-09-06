//go:build linux

package state

import (
	"errors"
	"os"
	"path/filepath"
	"testing"
	"time"
)

func TestSeparateStoreInstancesShareFilesystemLock(t *testing.T) {
	dir := t.TempDir()
	st1, err := Open(dir)
	if err != nil {
		t.Fatal(err)
	}
	st2, err := Open(dir)
	if err != nil {
		t.Fatal(err)
	}

	if err := lockStateFile(st1.lockFile); err != nil {
		t.Fatal(err)
	}
	done := make(chan error, 1)
	go func() {
		c := &Container{ID: "ctr-lock", Status: StatusCreated, CreatedAt: time.Now()}
		done <- st2.Save(c)
	}()

	select {
	case err := <-done:
		_ = unlockStateFile(st1.lockFile)
		t.Fatalf("second Store bypassed held flock: %v", err)
	case <-time.After(75 * time.Millisecond):
		// Expected: the separate file description is blocked on LOCK_EX.
	}

	if err := unlockStateFile(st1.lockFile); err != nil {
		t.Fatal(err)
	}
	select {
	case err := <-done:
		if err != nil {
			t.Fatalf("blocked Save failed after unlock: %v", err)
		}
	case <-time.After(2 * time.Second):
		t.Fatal("blocked Save did not resume after unlock")
	}
}

func TestOpenRejectsSymlinkStateLock(t *testing.T) {
	dir := t.TempDir()
	outside := filepath.Join(t.TempDir(), "outside-lock")
	if err := os.WriteFile(outside, []byte("do-not-touch"), 0o600); err != nil {
		t.Fatal(err)
	}
	if err := os.Symlink(outside, filepath.Join(dir, ".state.lock")); err != nil {
		t.Fatal(err)
	}
	if _, err := Open(dir); err == nil {
		t.Fatal("Open followed symlinked state lock")
	}
	data, err := os.ReadFile(outside)
	if err != nil {
		t.Fatal(err)
	}
	if string(data) != "do-not-touch" {
		t.Fatalf("outside lock target was modified: %q", data)
	}
}

func TestSaveReportsRevisionConflictThroughErrorsIs(t *testing.T) {
	dir := t.TempDir()
	a, err := Open(dir)
	if err != nil {
		t.Fatal(err)
	}
	b, err := Open(dir)
	if err != nil {
		t.Fatal(err)
	}
	seed := &Container{ID: "ctr-errors-is", Status: StatusCreated, CreatedAt: time.Now()}
	if err := a.Save(seed); err != nil {
		t.Fatal(err)
	}
	stale, err := b.Get(seed.ID)
	if err != nil {
		t.Fatal(err)
	}
	seed.Hostname = "new"
	if err := a.Save(seed); err != nil {
		t.Fatal(err)
	}
	stale.Hostname = "old"
	err = b.Save(stale)
	if !errors.Is(err, ErrRevisionConflict) {
		t.Fatalf("errors.Is revision conflict=false for %v", err)
	}
}
