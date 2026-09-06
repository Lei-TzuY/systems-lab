//go:build linux

package events

import (
	"os"
	"path/filepath"
	"testing"
)

func TestEventLogRejectsSymlinkTargetForAppend(t *testing.T) {
	dir := t.TempDir()
	victim := filepath.Join(dir, "victim")
	if err := os.WriteFile(victim, []byte("unchanged"), 0o600); err != nil {
		t.Fatal(err)
	}
	logPath := filepath.Join(dir, "events.log")
	if err := os.Symlink(victim, logPath); err != nil {
		t.Fatal(err)
	}

	if f, err := openEventLogForAppend(logPath); err == nil {
		f.Close()
		t.Fatal("expected symlink event log to be rejected")
	}
	got, err := os.ReadFile(victim)
	if err != nil {
		t.Fatal(err)
	}
	if string(got) != "unchanged" {
		t.Fatalf("symlink target was modified: %q", got)
	}
}

func TestEventLogRejectsSymlinkTargetForRead(t *testing.T) {
	dir := t.TempDir()
	victim := filepath.Join(dir, "victim")
	if err := os.WriteFile(victim, []byte("secret"), 0o600); err != nil {
		t.Fatal(err)
	}
	logPath := filepath.Join(dir, "events.log")
	if err := os.Symlink(victim, logPath); err != nil {
		t.Fatal(err)
	}

	if f, err := openEventLogForRead(logPath); err == nil {
		f.Close()
		t.Fatal("expected symlink event log to be rejected")
	}
}

func TestEventLogRejectsNonRegularTarget(t *testing.T) {
	path := filepath.Join(t.TempDir(), "events.log")
	if err := os.Mkdir(path, 0o700); err != nil {
		t.Fatal(err)
	}
	if f, err := openEventLogForAppend(path); err == nil {
		f.Close()
		t.Fatal("expected directory target to be rejected")
	}
}

func TestEventLogRejectsNonPrivatePermissionsForRead(t *testing.T) {
	path := filepath.Join(t.TempDir(), "events.log")
	if err := os.WriteFile(path, []byte("{}\n"), 0o666); err != nil {
		t.Fatal(err)
	}
	if err := os.Chmod(path, 0o666); err != nil {
		t.Fatal(err)
	}

	if f, err := openEventLogForRead(path); err == nil {
		f.Close()
		t.Fatal("expected world-writable event log to be rejected for read")
	}
}

func TestEventLogRejectsHardLinkedTarget(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "events.log")
	if err := os.WriteFile(path, []byte("{}\n"), 0o600); err != nil {
		t.Fatal(err)
	}
	alias := filepath.Join(dir, "events.log.alias")
	if err := os.Link(path, alias); err != nil {
		t.Fatal(err)
	}

	if f, err := openEventLogForRead(path); err == nil {
		f.Close()
		t.Fatal("expected hard-linked event log to be rejected for read")
	}
	if f, err := openEventLogForAppend(path); err == nil {
		f.Close()
		t.Fatal("expected hard-linked event log to be rejected for append")
	}
}

func TestEventLogAcceptsPrivateSingleLinkTarget(t *testing.T) {
	path := filepath.Join(t.TempDir(), "events.log")
	if err := os.WriteFile(path, []byte("{}\n"), 0o600); err != nil {
		t.Fatal(err)
	}

	readFile, err := openEventLogForRead(path)
	if err != nil {
		t.Fatalf("open private event log for read: %v", err)
	}
	if err := readFile.Close(); err != nil {
		t.Fatal(err)
	}

	appendFile, err := openEventLogForAppend(path)
	if err != nil {
		t.Fatalf("open private event log for append: %v", err)
	}
	if err := appendFile.Close(); err != nil {
		t.Fatal(err)
	}
}
