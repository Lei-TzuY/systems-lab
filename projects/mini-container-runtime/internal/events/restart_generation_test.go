package events

import (
	"errors"
	"os"
	"testing"
)

func TestRuntimeRestartGenerationsProduceDistinctStartDiePairs(t *testing.T) {
	resetLifecycleStagingForTest(t)
	t.Setenv("HOME", t.TempDir())

	const id = "ctr-restart-events"
	// Legacy cmdRun may pre-stage the first start. Runtime admission must adopt
	// that exact intent rather than creating a duplicate proof.
	if err := Publish(EventStart, id, "/rootfs", "started container"); err != nil {
		t.Fatalf("legacy pre-stage: %v", err)
	}
	if err := StageRuntimeStart(id, "/rootfs", "started container"); err != nil {
		t.Fatalf("runtime handoff: %v", err)
	}
	if err := CommitPendingStart(); err != nil {
		t.Fatalf("commit generation 1: %v", err)
	}
	if err := Publish(EventDie, id, "/rootfs", "exited with code 1"); err != nil {
		t.Fatalf("die generation 1: %v", err)
	}

	if err := StageRuntimeStart(id, "/rootfs", "started container"); err != nil {
		t.Fatalf("stage generation 2: %v", err)
	}
	if err := CommitPendingStart(); err != nil {
		t.Fatalf("commit generation 2: %v", err)
	}
	if err := Publish(EventDie, id, "/rootfs", "exited with code 0"); err != nil {
		t.Fatalf("die generation 2: %v", err)
	}

	got := readLifecycleEventsForTest(t)
	if len(got) != 4 {
		t.Fatalf("events=%+v, want four generation events", got)
	}
	want := []EventType{EventStart, EventDie, EventStart, EventDie}
	for i := range want {
		if got[i].Type != want[i] || got[i].ContainerID != id {
			t.Fatalf("event[%d]=%+v, want %s for %s", i, got[i], want[i], id)
		}
	}
}

func TestCancelPendingRuntimeStartPreventsFalseStart(t *testing.T) {
	resetLifecycleStagingForTest(t)
	t.Setenv("HOME", t.TempDir())

	const id = "ctr-preexec-abort"
	if err := StageRuntimeStart(id, "/rootfs", "started container"); err != nil {
		t.Fatal(err)
	}
	CancelPendingStart(id)
	if err := CommitPendingStart(); err != nil {
		t.Fatalf("commit after cancel: %v", err)
	}
	if _, err := os.Stat(LogPath()); !errors.Is(err, os.ErrNotExist) {
		t.Fatalf("cancelled pre-exec attempt produced event log: %v", err)
	}
}

func TestDieAppendFailureDoesNotWedgeNextGenerationProof(t *testing.T) {
	resetLifecycleStagingForTest(t)
	t.Setenv("HOME", t.TempDir())

	const id = "ctr-die-log-failure"
	if err := StageRuntimeStart(id, "/rootfs", "started container"); err != nil {
		t.Fatal(err)
	}
	if err := CommitPendingStart(); err != nil {
		t.Fatal(err)
	}

	// Replace events.log with a directory so the Die append deterministically
	// fails after the generation already exited.
	if err := os.Remove(LogPath()); err != nil {
		t.Fatal(err)
	}
	if err := os.Mkdir(LogPath(), 0o700); err != nil {
		t.Fatal(err)
	}
	if err := Publish(EventDie, id, "/rootfs", "exited"); err == nil {
		t.Fatal("die append unexpectedly succeeded against directory path")
	}
	if err := os.Remove(LogPath()); err != nil {
		t.Fatal(err)
	}

	if err := StageRuntimeStart(id, "/rootfs", "started container"); err != nil {
		t.Fatalf("next generation remained wedged after die log failure: %v", err)
	}
	CancelPendingStart(id)
}
