package events

import (
	"bytes"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestManagedEventStorageRejectsSymlinkedStateRoot(t *testing.T) {
	home := t.TempDir()
	t.Setenv("HOME", home)

	outside := t.TempDir()
	if err := os.Chmod(outside, 0o755); err != nil {
		t.Fatal(err)
	}
	const secret = "{\"timestamp\":\"2026-01-01T00:00:00Z\",\"type\":\"create\",\"container_id\":\"outside-secret\",\"message\":\"HOST-SECRET\"}\n"
	victim := filepath.Join(outside, "events.log")
	if err := os.WriteFile(victim, []byte(secret), 0o600); err != nil {
		t.Fatal(err)
	}
	if err := os.Symlink(outside, filepath.Join(home, ".minicontainer")); err != nil {
		t.Skipf("symlink unavailable: %v", err)
	}

	if err := Publish(EventStart, "new-container", "", "must-not-escape"); err == nil {
		t.Fatal("Publish accepted symlinked event state root")
	}
	data, err := os.ReadFile(victim)
	if err != nil || string(data) != secret {
		t.Fatalf("outside event log changed: data=%q err=%v", data, err)
	}
	info, err := os.Stat(outside)
	if err != nil {
		t.Fatal(err)
	}
	if got := info.Mode().Perm(); got != 0o755 {
		t.Fatalf("outside directory mode changed to %o, want 755", got)
	}

	var out bytes.Buffer
	if err := StreamEvents(false, &out); err == nil {
		t.Fatal("StreamEvents accepted symlinked event state root")
	}
	if strings.Contains(out.String(), "HOST-SECRET") || strings.Contains(out.String(), "outside-secret") {
		t.Fatalf("StreamEvents leaked outside event log: %q", out.String())
	}
}

func TestManagedEventStorageCreatesPrivateRealFiles(t *testing.T) {
	home := t.TempDir()
	t.Setenv("HOME", home)
	const containerID = "abcdef0123456789"
	persistCreateGuardContainer(t, containerID)

	if err := Publish(EventCreate, containerID, "image", "created"); err != nil {
		t.Fatalf("Publish: %v", err)
	}

	root := eventStateDir()
	rootInfo, err := os.Lstat(root)
	if err != nil {
		t.Fatal(err)
	}
	if rootInfo.Mode()&os.ModeSymlink != 0 || !rootInfo.IsDir() {
		t.Fatalf("event state root is not a real directory: %v", rootInfo.Mode())
	}
	if got := rootInfo.Mode().Perm(); got != 0o700 {
		t.Fatalf("event state root mode=%o, want 700", got)
	}

	logInfo, err := os.Lstat(LogPath())
	if err != nil {
		t.Fatal(err)
	}
	if !logInfo.Mode().IsRegular() || logInfo.Mode().Perm() != 0o600 {
		t.Fatalf("event log mode=%v, want regular 0600", logInfo.Mode())
	}

	var out bytes.Buffer
	if err := StreamEvents(false, &out); err != nil {
		t.Fatalf("StreamEvents: %v", err)
	}
	if !strings.Contains(out.String(), containerID[:12]) {
		t.Fatalf("event stream missing shortened container ID: %q", out.String())
	}
}
