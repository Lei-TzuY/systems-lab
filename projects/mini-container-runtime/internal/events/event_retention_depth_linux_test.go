//go:build linux

package events

import (
	"bytes"
	"io"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func writeSizedEventGeneration(t *testing.T, path, prefix string) {
	t.Helper()
	data := bytes.Repeat([]byte{'\n'}, int(maxEventLogBytes))
	copy(data, prefix)
	if err := os.WriteFile(path, data, 0o600); err != nil {
		t.Fatal(err)
	}
}

func readGenerationPrefix(t *testing.T, path string, n int) string {
	t.Helper()
	f, err := os.Open(path)
	if err != nil {
		t.Fatal(err)
	}
	defer f.Close()
	buf := make([]byte, n)
	got, err := io.ReadFull(f, buf)
	if err != nil {
		t.Fatal(err)
	}
	return string(buf[:got])
}

func TestRotateEventLogRetainsTwoGenerationsOldestFirst(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "events.log")

	writeSizedEventGeneration(t, path, "generation-zero")
	if err := rotateEventLogIfNeeded(path); err != nil {
		t.Fatalf("first rotation: %v", err)
	}
	writeSizedEventGeneration(t, path, "generation-one")
	if err := rotateEventLogIfNeeded(path); err != nil {
		t.Fatalf("second rotation: %v", err)
	}
	if err := os.WriteFile(path, []byte("generation-two"), 0o600); err != nil {
		t.Fatal(err)
	}

	if got := readGenerationPrefix(t, path+".2", len("generation-zero")); got != "generation-zero" {
		t.Fatalf("older retained prefix=%q, want generation-zero", got)
	}
	if got := readGenerationPrefix(t, path+".1", len("generation-one")); got != "generation-one" {
		t.Fatalf("newer retained prefix=%q, want generation-one", got)
	}
	if got := readGenerationPrefix(t, path, len("generation-two")); got != "generation-two" {
		t.Fatalf("active prefix=%q, want generation-two", got)
	}

	snapshot, err := openEventLogFollowStartupSnapshot(path)
	if err != nil {
		t.Fatalf("follow startup snapshot: %v", err)
	}
	defer snapshot.close()
	if len(snapshot.retained) != retainedEventLogGenerations {
		t.Fatalf("retained snapshots=%d, want %d", len(snapshot.retained), retainedEventLogGenerations)
	}
	for i, want := range []string{"generation-zero", "generation-one"} {
		buf := make([]byte, len(want))
		if _, err := io.ReadFull(snapshot.retained[i].file, buf); err != nil {
			t.Fatalf("read retained snapshot %d: %v", i, err)
		}
		if string(buf) != want {
			t.Fatalf("retained snapshot %d=%q, want %q", i, buf, want)
		}
	}
	if snapshot.active == nil {
		t.Fatal("active snapshot is nil")
	}
	buf := make([]byte, len("generation-two"))
	if _, err := io.ReadFull(snapshot.active, buf); err != nil {
		t.Fatalf("read active snapshot: %v", err)
	}
	if string(buf) != "generation-two" {
		t.Fatalf("active snapshot=%q, want generation-two", buf)
	}
}

func TestRotateEventLogRejectsUnsafeRetainedSymlink(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "events.log")
	victim := filepath.Join(dir, "victim")
	victimData := []byte("do-not-touch")
	if err := os.WriteFile(victim, victimData, 0o600); err != nil {
		t.Fatal(err)
	}
	if err := os.Symlink(victim, path+".1"); err != nil {
		t.Fatal(err)
	}
	writeSizedEventGeneration(t, path, "active-generation")

	err := rotateEventLogIfNeeded(path)
	if err == nil {
		t.Fatal("rotation unexpectedly accepted symlinked retained generation")
	}
	if !strings.Contains(err.Error(), "retained event log") && !strings.Contains(err.Error(), "event log") {
		t.Fatalf("unexpected rotation error: %v", err)
	}
	gotVictim, err := os.ReadFile(victim)
	if err != nil {
		t.Fatal(err)
	}
	if !bytes.Equal(gotVictim, victimData) {
		t.Fatalf("victim changed: %q", gotVictim)
	}
	if _, err := os.Lstat(path); err != nil {
		t.Fatalf("active generation changed despite failed rotation: %v", err)
	}
	if _, err := os.Lstat(path + ".2"); !os.IsNotExist(err) {
		t.Fatalf("older retained path created during failed rotation: %v", err)
	}
}
