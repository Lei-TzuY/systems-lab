//go:build linux

package state

import (
	"os"
	"path/filepath"
	"testing"
	"time"
)

func TestExitedIdentityReadRejectsSymlink(t *testing.T) {
	st := newLifecycleTestStore(t, "ctr-exit-symlink-read")
	victim := filepath.Join(t.TempDir(), "victim.json")
	if err := os.WriteFile(victim, []byte(`{"pid":123,"pid_start_time":456}`), 0o600); err != nil {
		t.Fatal(err)
	}
	path := exitedIdentityPath(st.ctrDir, "ctr-exit-symlink-read")
	if err := os.Symlink(victim, path); err != nil {
		t.Fatal(err)
	}
	if _, _, err := st.readExitedIdentityUnlocked("ctr-exit-symlink-read"); err == nil {
		t.Fatal("expected symlink legacy exited-identity sidecar to be rejected")
	}
}

func TestModernStopRemovesSymlinkSidecarWithoutTouchingTarget(t *testing.T) {
	const id = "ctr-exit-symlink-write"
	st := newLifecycleTestStore(t, id)
	if err := st.MarkRunning(id, 123, 456, time.Now()); err != nil {
		t.Fatal(err)
	}

	victim := filepath.Join(t.TempDir(), "victim")
	if err := os.WriteFile(victim, []byte("unchanged"), 0o600); err != nil {
		t.Fatal(err)
	}
	path := exitedIdentityPath(st.ctrDir, id)
	if err := os.Symlink(victim, path); err != nil {
		t.Fatal(err)
	}

	changed, err := st.MarkStoppedIfIdentity(id, 123, 456, -1, time.Now())
	if err != nil || !changed {
		t.Fatalf("unknown stop: changed=%v err=%v", changed, err)
	}
	got, err := os.ReadFile(victim)
	if err != nil {
		t.Fatal(err)
	}
	if string(got) != "unchanged" {
		t.Fatalf("symlink target was modified: %q", got)
	}
	if _, err := os.Lstat(path); !os.IsNotExist(err) {
		t.Fatalf("legacy sidecar symlink survived modern stop: %v", err)
	}
	pid, start, ok, err := st.GetExitedIdentity(id)
	if err != nil || !ok || pid != 123 || start != 456 {
		t.Fatalf("embedded identity missing after symlink cleanup: pid=%d start=%d ok=%v err=%v", pid, start, ok, err)
	}
}
