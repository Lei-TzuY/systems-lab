//go:build linux

package events

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestEventLogAppendRotatesAtRetentionLimit(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "events.log")
	if err := os.WriteFile(path, []byte(strings.Repeat("x\n", int(maxEventLogBytes/2))), 0o600); err != nil {
		t.Fatal(err)
	}

	f, err := openEventLogForAppend(path)
	if err != nil {
		t.Fatalf("open writer after rotation: %v", err)
	}
	if _, err := f.Write([]byte("new-generation\n")); err != nil {
		_ = f.Close()
		t.Fatalf("write new generation: %v", err)
	}
	if err := f.Sync(); err != nil {
		_ = f.Close()
		t.Fatalf("sync new generation: %v", err)
	}
	if err := f.Close(); err != nil {
		t.Fatal(err)
	}

	rotated, err := os.Stat(path + ".1")
	if err != nil {
		t.Fatalf("stat retained generation: %v", err)
	}
	if rotated.Size() != maxEventLogBytes {
		t.Fatalf("retained size=%d want=%d", rotated.Size(), maxEventLogBytes)
	}
	current, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	if string(current) != "new-generation\n" {
		t.Fatalf("active generation=%q", current)
	}
}

func TestEventLogRotationReplacesOnlyPreviousGeneration(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "events.log")
	if err := os.WriteFile(path, []byte(strings.Repeat("x\n", int(maxEventLogBytes/2))), 0o600); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(path+".1", []byte("stale\n"), 0o600); err != nil {
		t.Fatal(err)
	}

	f, err := openEventLogForAppend(path)
	if err != nil {
		t.Fatalf("rotate over previous generation: %v", err)
	}
	if err := f.Close(); err != nil {
		t.Fatal(err)
	}

	retained, err := os.ReadFile(path + ".1")
	if err != nil {
		t.Fatal(err)
	}
	if strings.HasPrefix(string(retained), "stale") {
		t.Fatal("stale retained generation survived replacement")
	}
	info, err := os.Stat(path + ".1")
	if err != nil {
		t.Fatal(err)
	}
	if info.Size() != maxEventLogBytes {
		t.Fatalf("retained size=%d want=%d", info.Size(), maxEventLogBytes)
	}
}

func TestEventLogRotationRejectsSymlinkSourceWithoutTouchingRetained(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "events.log")
	victim := filepath.Join(dir, "victim")
	if err := os.WriteFile(victim, []byte("victim\n"), 0o600); err != nil {
		t.Fatal(err)
	}
	if err := os.Symlink(victim, path); err != nil {
		t.Fatal(err)
	}
	const retained = "retained-safe\n"
	if err := os.WriteFile(path+".1", []byte(retained), 0o600); err != nil {
		t.Fatal(err)
	}

	f, err := openEventLogForAppend(path)
	if f != nil {
		_ = f.Close()
	}
	if err == nil {
		t.Fatal("expected symlink rotation source to fail closed")
	}
	got, readErr := os.ReadFile(path + ".1")
	if readErr != nil {
		t.Fatal(readErr)
	}
	if string(got) != retained {
		t.Fatalf("retained generation changed on rejected rotation: %q", got)
	}
}
