//go:build linux

package container

import (
	"errors"
	"os/exec"
	"testing"
	"time"

	"minicontainer/internal/state"
)

func payloadExitError(t *testing.T, code string) *exec.ExitError {
	t.Helper()
	err := exec.Command("sh", "-c", "exit "+code).Run()
	var exitErr *exec.ExitError
	if !errors.As(err, &exitErr) {
		t.Fatalf("expected *exec.ExitError, got %T: %v", err, err)
	}
	return exitErr
}

func TestFinalizeManagedParentExitUsesGenerationFinalizerWithoutCgroupOwnership(t *testing.T) {
	st, err := state.Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	snapshot := &state.Container{
		ID:           "ctr-unowned-cgroup",
		Status:       state.StatusRunning,
		PID:          4242,
		PIDStartTime: 99,
		CreatedAt:    time.Now(),
	}
	if err := st.Save(snapshot); err != nil {
		t.Fatal(err)
	}

	finalizerCalled := false
	if err := finalizeManagedParentExit(
		st,
		snapshot,
		7,
		time.Now(),
		false,
		func(gotStore *state.Store, got *state.Container, exitCode int, finished time.Time) (bool, error) {
			finalizerCalled = true
			return gotStore.MarkStoppedIfIdentity(got.ID, got.PID, got.PIDStartTime, exitCode, finished)
		},
	); err != nil {
		t.Fatalf("finalize unowned managed exit: %v", err)
	}
	if !finalizerCalled {
		t.Fatal("generation finalizer skipped because cgroup Apply failed")
	}

	current, err := st.Get(snapshot.ID)
	if err != nil {
		t.Fatal(err)
	}
	if current.Status != state.StatusStopped || current.PID != 0 || current.PIDStartTime != 0 {
		t.Fatalf("state not reconciled without cgroup ownership: %+v", current)
	}
	if current.ExitCode != 7 || current.FinishedAt == nil {
		t.Fatalf("exit metadata lost without cgroup ownership: exit=%d finished=%v", current.ExitCode, current.FinishedAt)
	}
}

func TestFinalizeManagedParentExitUsesGenerationFinalizerWhenOwned(t *testing.T) {
	st, err := state.Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	snapshot := &state.Container{ID: "ctr-owned", PID: 1234, PIDStartTime: 55}
	finishedAt := time.Now()
	calls := 0

	if err := finalizeManagedParentExit(
		st,
		snapshot,
		9,
		finishedAt,
		true,
		func(gotStore *state.Store, got *state.Container, exitCode int, finished time.Time) (bool, error) {
			calls++
			if gotStore != st || got != snapshot || exitCode != 9 || !finished.Equal(finishedAt) {
				t.Fatalf("wrong generation finalizer arguments: store=%p snapshot=%+v exit=%d finished=%v", gotStore, got, exitCode, finished)
			}
			return false, nil
		},
	); err != nil {
		t.Fatalf("finalize owned managed exit: %v", err)
	}
	if calls != 1 {
		t.Fatalf("generation finalizer calls=%d, want 1", calls)
	}
}

func TestParentExitResultPreservesPayloadExitAndMarksCleanupFailureRuntimeControl(t *testing.T) {
	payloadErr := payloadExitError(t, "17")
	cleanupErr := errors.New("cgroup still populated")

	err := parentExitResult(payloadErr, cleanupErr, nil)
	if err == nil {
		t.Fatal("expected combined error")
	}
	if !errors.Is(err, cleanupErr) {
		t.Fatalf("cleanup error lost: %v", err)
	}
	var gotExit *exec.ExitError
	if !errors.As(err, &gotExit) || gotExit.ExitCode() != 17 {
		t.Fatalf("payload exit status lost: exit=%v err=%v", gotExit, err)
	}
	if !isRuntimeControlError(err) {
		t.Fatalf("cleanup failure must block restart: %v", err)
	}
}

func TestParentExitResultBridgeCleanupFailureBlocksRestart(t *testing.T) {
	bridgeErr := &runtimeSetupError{err: errors.New("remove veth")}
	err := parentExitResult(nil, nil, bridgeErr)
	if !errors.Is(err, bridgeErr) {
		t.Fatalf("bridge cleanup error lost: %v", err)
	}
	if !isRuntimeControlError(err) {
		t.Fatalf("bridge cleanup failure must block restart: %v", err)
	}
}

func TestParentExitResultCleanExitIsNil(t *testing.T) {
	if err := parentExitResult(nil, nil, nil); err != nil {
		t.Fatalf("clean exit returned error: %v", err)
	}
}
