package events

import (
	"errors"
	"os"
	"reflect"
	"strings"
	"testing"
	"time"
)

func resetExecStagingForTest(t *testing.T) {
	t.Helper()
	mu.Lock()
	stagedExecs = make(map[string]stagedExecEvent)
	activeExecs = make(map[string]stagedExecEvent)
	mu.Unlock()
	t.Cleanup(func() {
		mu.Lock()
		stagedExecs = make(map[string]stagedExecEvent)
		activeExecs = make(map[string]stagedExecEvent)
		mu.Unlock()
	})
}

func TestExecEventIsStagedUntilPayloadStartCommit(t *testing.T) {
	resetExecStagingForTest(t)
	t.Setenv("HOME", t.TempDir())

	if err := Publish(EventExec, "ctr-exec", "rootfs", "exec [true]"); err != nil {
		t.Fatalf("stage exec: %v", err)
	}
	if _, err := os.Stat(LogPath()); !errors.Is(err, os.ErrNotExist) {
		t.Fatalf("exec event log exists before payload proof: err=%v", err)
	}

	commitFloor := time.Now()
	if err := CommitPendingExec(); err != nil {
		t.Fatalf("commit exec: %v", err)
	}
	got := readLifecycleEventsForTest(t)
	if len(got) != 1 || got[0].Type != EventExec || got[0].ContainerID != "ctr-exec" {
		t.Fatalf("events=%+v, want one committed exec", got)
	}
	if got[0].Timestamp.Before(commitFloor) {
		t.Fatalf("exec timestamp=%v predates payload-start floor=%v", got[0].Timestamp, commitFloor)
	}
}

func TestExecGenerationAttributionSurvivesStartAndTerminalOutcome(t *testing.T) {
	resetExecStagingForTest(t)
	t.Setenv("HOME", t.TempDir())
	command := []string{"sh", "-c", "exit 7"}
	if err := Publish(EventExec, "ctr-gen", "rootfs", "exec [sh -c exit 7]"); err != nil {
		t.Fatal(err)
	}
	if err := BindPendingExecAttribution(4321, 987654, command); err != nil {
		t.Fatal(err)
	}
	// Mutating caller-owned argv after binding must not rewrite durable audit
	// attribution while the event is waiting for payload-start proof.
	command[0] = "mutated"
	if err := CommitPendingExec(); err != nil {
		t.Fatal(err)
	}
	if err := CompletePendingExec(7, ""); err != nil {
		t.Fatal(err)
	}
	got := readLifecycleEventsForTest(t)
	if len(got) != 2 {
		t.Fatalf("events=%+v", got)
	}
	wantCommand := []string{"sh", "-c", "exit 7"}
	for _, evt := range got {
		if evt.ContainerPID != 4321 || evt.ContainerPIDStartTime != 987654 || !reflect.DeepEqual(evt.Command, wantCommand) {
			t.Fatalf("generation attribution lost across lifecycle: %+v", evt)
		}
	}
}

func TestBindPendingExecAttributionRejectsInvalidGenerationWithoutMutation(t *testing.T) {
	resetExecStagingForTest(t)
	if err := Publish(EventExec, "ctr-invalid-gen", "rootfs", "exec [true]"); err != nil {
		t.Fatal(err)
	}
	if err := BindPendingExecAttribution(0, 123, []string{"true"}); err == nil {
		t.Fatal("invalid generation unexpectedly accepted")
	}
	mu.Lock()
	staged := stagedExecs["ctr-invalid-gen"]
	mu.Unlock()
	if staged.containerPID != 0 || staged.pidStartTime != 0 || staged.command != nil {
		t.Fatalf("invalid bind partially mutated staged attribution: %+v", staged)
	}
}

func TestCommitPendingExecRetainsStagedAttributionWhenDurableAppendFails(t *testing.T) {
	resetExecStagingForTest(t)
	t.Setenv("HOME", t.TempDir())

	if err := Publish(EventExec, "ctr-retry", "rootfs", "exec [true]"); err != nil {
		t.Fatal(err)
	}
	if err := os.Mkdir(LogPath(), 0o700); err != nil {
		t.Fatalf("block event log path: %v", err)
	}
	if err := CommitPendingExec(); err == nil {
		t.Fatal("commit unexpectedly succeeded with events.log as a directory")
	}

	mu.Lock()
	_, stillStaged := stagedExecs["ctr-retry"]
	_, becameActive := activeExecs["ctr-retry"]
	mu.Unlock()
	if !stillStaged || becameActive {
		t.Fatalf("append failure changed attribution: staged=%v active=%v", stillStaged, becameActive)
	}

	if err := os.Remove(LogPath()); err != nil {
		t.Fatalf("remove append blocker: %v", err)
	}
	if err := CommitPendingExec(); err != nil {
		t.Fatalf("retry durable exec commit: %v", err)
	}
	got := readLifecycleEventsForTest(t)
	if len(got) != 1 || got[0].Type != EventExec || got[0].ContainerID != "ctr-retry" {
		t.Fatalf("events after retry=%+v, want one durable exec start", got)
	}
}

func TestCompletePendingExecRecordsExactlyOneTerminalOutcome(t *testing.T) {
	resetExecStagingForTest(t)
	t.Setenv("HOME", t.TempDir())

	if err := Publish(EventExec, "ctr-exit", "rootfs", "exec [sh -c false]"); err != nil {
		t.Fatal(err)
	}
	if err := CommitPendingExec(); err != nil {
		t.Fatal(err)
	}
	if err := CompletePendingExec(17, ""); err != nil {
		t.Fatal(err)
	}
	if err := CompletePendingExec(99, "duplicate"); err != nil {
		t.Fatal(err)
	}

	got := readLifecycleEventsForTest(t)
	if len(got) != 2 || got[0].Type != EventExec || got[1].Type != EventExecExit {
		t.Fatalf("events=%+v, want exec then exec_exit", got)
	}
	if got[1].ContainerID != "ctr-exit" || !strings.Contains(got[1].Message, "exit_code=17") {
		t.Fatalf("terminal event=%+v", got[1])
	}
}

func TestFailPendingExecRecordsFailureWithoutStartedEvent(t *testing.T) {
	resetExecStagingForTest(t)
	t.Setenv("HOME", t.TempDir())

	if err := Publish(EventExec, "ctr-fail", "rootfs", "exec [missing]"); err != nil {
		t.Fatal(err)
	}
	if err := FailPendingExec("payload start not proven"); err != nil {
		t.Fatal(err)
	}

	got := readLifecycleEventsForTest(t)
	if len(got) != 1 || got[0].Type != EventExecFailed || got[0].ContainerID != "ctr-fail" {
		t.Fatalf("events=%+v, want one exec_failed", got)
	}
	if !strings.Contains(got[0].Message, "payload start not proven") {
		t.Fatalf("failure event lost cause: %+v", got[0])
	}
}

func TestDiscardPendingExecSuppressesFailedSetup(t *testing.T) {
	resetExecStagingForTest(t)
	t.Setenv("HOME", t.TempDir())

	if err := Publish(EventExec, "ctr-discard", "rootfs", "exec [missing]"); err != nil {
		t.Fatal(err)
	}
	if err := DiscardPendingExec(); err != nil {
		t.Fatalf("discard exec: %v", err)
	}
	if err := CommitPendingExec(); err != nil {
		t.Fatalf("commit after discard should be no-op: %v", err)
	}
	if _, err := os.Stat(LogPath()); !errors.Is(err, os.ErrNotExist) {
		t.Fatalf("discarded exec wrote event log: err=%v", err)
	}
}

func TestCommitPendingExecRejectsAmbiguousContainers(t *testing.T) {
	resetExecStagingForTest(t)
	t.Setenv("HOME", t.TempDir())

	if err := Publish(EventExec, "ctr-a", "rootfs-a", "exec a"); err != nil {
		t.Fatal(err)
	}
	if err := Publish(EventExec, "ctr-b", "rootfs-b", "exec b"); err != nil {
		t.Fatal(err)
	}
	err := CommitPendingExec()
	if err == nil || !strings.Contains(err.Error(), "2 staged exec events") {
		t.Fatalf("ambiguous exec commit error=%v", err)
	}
	if _, statErr := os.Stat(LogPath()); !errors.Is(statErr, os.ErrNotExist) {
		t.Fatalf("ambiguous exec commit wrote event log: err=%v", statErr)
	}
}
