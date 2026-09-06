package events

import (
	"encoding/json"
	"errors"
	"io"
	"os"
	"strings"
	"testing"
	"time"
)

func resetLifecycleStagingForTest(t *testing.T) {
	t.Helper()
	mu.Lock()
	stagedStarts = make(map[string]stagedStartEvent)
	committedStarts = make(map[string]struct{})
	mu.Unlock()
	t.Cleanup(func() {
		mu.Lock()
		stagedStarts = make(map[string]stagedStartEvent)
		committedStarts = make(map[string]struct{})
		mu.Unlock()
	})
}

func readLifecycleEventsForTest(t *testing.T) []Event {
	t.Helper()
	f, err := os.Open(LogPath())
	if err != nil {
		t.Fatal(err)
	}
	defer f.Close()

	var out []Event
	dec := json.NewDecoder(f)
	for {
		var evt Event
		if err := dec.Decode(&evt); err != nil {
			if errors.Is(err, io.EOF) {
				return out
			}
			t.Fatal(err)
		}
		out = append(out, evt)
	}
}

func TestStartEventIsStagedUntilRuntimeCommit(t *testing.T) {
	resetLifecycleStagingForTest(t)
	t.Setenv("HOME", t.TempDir())

	if err := Publish(EventStart, "ctr-stage", "rootfs", "started container"); err != nil {
		t.Fatalf("stage start: %v", err)
	}
	if _, err := os.Stat(LogPath()); !errors.Is(err, os.ErrNotExist) {
		t.Fatalf("event log exists before runtime commit: err=%v", err)
	}

	commitFloor := time.Now()
	if err := CommitPendingStart(); err != nil {
		t.Fatalf("commit start: %v", err)
	}
	events := readLifecycleEventsForTest(t)
	if len(events) != 1 || events[0].Type != EventStart || events[0].ContainerID != "ctr-stage" {
		t.Fatalf("events=%+v, want one committed start", events)
	}
	if events[0].Timestamp.Before(commitFloor) {
		t.Fatalf("start timestamp=%v predates runtime commit floor=%v", events[0].Timestamp, commitFloor)
	}
}

func TestDieWithoutCommittedStartIsSuppressedAndClearsPending(t *testing.T) {
	resetLifecycleStagingForTest(t)
	t.Setenv("HOME", t.TempDir())

	if err := Publish(EventStart, "ctr-abort", "rootfs", "started container"); err != nil {
		t.Fatal(err)
	}
	if err := Publish(EventDie, "ctr-abort", "rootfs", "exited with code 1"); err != nil {
		t.Fatalf("suppress pre-release die: %v", err)
	}
	if _, err := os.Stat(LogPath()); !errors.Is(err, os.ErrNotExist) {
		t.Fatalf("pre-release start/die produced a log: err=%v", err)
	}
	if err := CommitPendingStart(); err != nil {
		t.Fatalf("cleared pending start should make commit a no-op: %v", err)
	}
}

func TestCommittedStartAllowsExactlyOneDie(t *testing.T) {
	resetLifecycleStagingForTest(t)
	t.Setenv("HOME", t.TempDir())

	if err := Publish(EventStart, "ctr-pair", "rootfs", "started container"); err != nil {
		t.Fatal(err)
	}
	if err := CommitPendingStart(); err != nil {
		t.Fatal(err)
	}
	if err := Publish(EventDie, "ctr-pair", "rootfs", "exited with code 7"); err != nil {
		t.Fatal(err)
	}
	if err := Publish(EventDie, "ctr-pair", "rootfs", "duplicate die"); err != nil {
		t.Fatal(err)
	}

	events := readLifecycleEventsForTest(t)
	if len(events) != 2 || events[0].Type != EventStart || events[1].Type != EventDie {
		t.Fatalf("events=%+v, want start then die", events)
	}
}

func TestCommitPendingStartRejectsAmbiguousContainers(t *testing.T) {
	resetLifecycleStagingForTest(t)
	t.Setenv("HOME", t.TempDir())

	if err := Publish(EventStart, "ctr-a", "rootfs-a", "start a"); err != nil {
		t.Fatal(err)
	}
	if err := Publish(EventStart, "ctr-b", "rootfs-b", "start b"); err != nil {
		t.Fatal(err)
	}
	err := CommitPendingStart()
	if err == nil || !strings.Contains(err.Error(), "2 staged start events") {
		t.Fatalf("ambiguous commit error=%v", err)
	}
	if _, statErr := os.Stat(LogPath()); !errors.Is(statErr, os.ErrNotExist) {
		t.Fatalf("ambiguous start commit wrote an event log: err=%v", statErr)
	}
}
