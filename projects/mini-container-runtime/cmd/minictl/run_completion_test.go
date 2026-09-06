package main

import (
	"errors"
	"strings"
	"testing"
	"time"

	"minicontainer/internal/state"
)

func saveRunCompletionContainer(t *testing.T, st *state.Store, id string) {
	t.Helper()
	if err := st.Save(&state.Container{ID: id, Status: state.StatusCreated}); err != nil {
		t.Fatalf("save container: %v", err)
	}
}

func TestSettleRunCommandStateUsesAuthoritativePayloadExit(t *testing.T) {
	st, err := state.Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	const id = "payload-exit"
	saveRunCompletionContainer(t, st, id)
	if err := st.MarkRunning(id, 4242, 88, time.Unix(10, 0)); err != nil {
		t.Fatal(err)
	}
	authoritativeFinished := time.Unix(11, 0)
	if changed, err := st.MarkStoppedIfIdentity(id, 4242, 88, 17, authoritativeFinished); err != nil || !changed {
		t.Fatalf("authoritative stop: changed=%v err=%v", changed, err)
	}

	runErr := errors.New("payload exited non-zero")
	got, err := settleRunCommandState(st, id, runErr, time.Unix(99, 0))
	if err != nil {
		t.Fatalf("settle: %v", err)
	}
	if got.Status != state.StatusStopped || got.ExitCode != 17 {
		t.Fatalf("state=%+v, want authoritative exit code 17", got)
	}
	if got.FinishedAt == nil || !got.FinishedAt.Equal(authoritativeFinished) {
		t.Fatalf("finished_at=%v, want authoritative %v", got.FinishedAt, authoritativeFinished)
	}
}

func TestSettleRunCommandStatePreservesCleanAuthoritativeExitOnRuntimeCleanupError(t *testing.T) {
	st, err := state.Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	const id = "cleanup-error"
	saveRunCompletionContainer(t, st, id)
	if err := st.MarkRunning(id, 5151, 99, time.Unix(20, 0)); err != nil {
		t.Fatal(err)
	}
	if changed, err := st.MarkStoppedIfIdentity(id, 5151, 99, 0, time.Unix(21, 0)); err != nil || !changed {
		t.Fatalf("authoritative stop: changed=%v err=%v", changed, err)
	}

	got, err := settleRunCommandState(st, id, errors.New("runtime teardown failed"), time.Unix(22, 0))
	if err != nil {
		t.Fatalf("settle: %v", err)
	}
	if got.Status != state.StatusStopped || got.ExitCode != 0 {
		t.Fatalf("state=%+v, want payload exit code 0 despite runtime error", got)
	}
}

func TestSettleRunCommandStateRejectsRuntimeErrorStillCreated(t *testing.T) {
	st, err := state.Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	const id = "prestart-failure"
	saveRunCompletionContainer(t, st, id)
	finishedAt := time.Unix(30, 0)

	got, err := settleRunCommandState(st, id, errors.New("cmd start failed"), finishedAt)
	if err == nil || !strings.Contains(err.Error(), "was not durably finalized") {
		t.Fatalf("error=%v, want runtime-finalization invariant failure", err)
	}
	if got == nil || got.Status != state.StatusCreated {
		t.Fatalf("state=%+v, want unchanged created state", got)
	}
	persisted, getErr := st.Get(id)
	if getErr != nil {
		t.Fatal(getErr)
	}
	if persisted.Status != state.StatusCreated || persisted.FinishedAt != nil {
		t.Fatalf("CLI mutated persisted state: %+v", persisted)
	}
}

func TestSettleRunCommandStateRejectsSuccessfulRunStillCreated(t *testing.T) {
	st, err := state.Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	const id = "success-created"
	saveRunCompletionContainer(t, st, id)

	got, err := settleRunCommandState(st, id, nil, time.Unix(40, 0))
	if err == nil || !strings.Contains(err.Error(), "never left created state") {
		t.Fatalf("error=%v, want created-state invariant failure", err)
	}
	if got == nil || got.Status != state.StatusCreated {
		t.Fatalf("state=%+v, want unchanged created state", got)
	}
	persisted, getErr := st.Get(id)
	if getErr != nil {
		t.Fatal(getErr)
	}
	if persisted.Status != state.StatusCreated || persisted.FinishedAt != nil {
		t.Fatalf("persisted state changed: %+v", persisted)
	}
}

func TestSettleRunCommandStateRejectsRuntimeReturnWhileRunning(t *testing.T) {
	st, err := state.Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	const id = "still-running"
	saveRunCompletionContainer(t, st, id)
	if err := st.MarkRunning(id, 6161, 101, time.Unix(50, 0)); err != nil {
		t.Fatal(err)
	}

	got, err := settleRunCommandState(st, id, errors.New("runtime returned"), time.Unix(51, 0))
	if err == nil || !strings.Contains(err.Error(), "remains running") {
		t.Fatalf("error=%v, want running-state invariant failure", err)
	}
	if got == nil || got.Status != state.StatusRunning || got.PID != 6161 || got.PIDStartTime != 101 {
		t.Fatalf("running state changed: %+v", got)
	}
}

func TestJoinRunCommandErrorsPreservesBothFailures(t *testing.T) {
	runErr := errors.New("runtime failure")
	stateErr := errors.New("state failure")
	joined := joinRunCommandErrors(runErr, stateErr)
	if !errors.Is(joined, runErr) || !errors.Is(joined, stateErr) {
		t.Fatalf("joined error=%v, want both causes", joined)
	}
	if got := joinRunCommandErrors(nil, nil); got != nil {
		t.Fatalf("nil join=%v, want nil", got)
	}
}
