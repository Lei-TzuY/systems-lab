package container

import (
	"errors"
	"testing"
	"time"

	"minicontainer/internal/state"
)

func TestReconcileDeadGenerationReturnsDurableStopAfterFinalizerPostCommitError(t *testing.T) {
	st, err := state.Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	c := saveReconcileContainer(t, st, &state.Container{
		ID:           "post-commit-finalize-error",
		Status:       state.StatusRunning,
		PID:          3141,
		PIDStartTime: 59,
	})
	sentinel := errors.New("cleanup token retained")
	cleanupCalls := 0

	got, err := reconcileContainerStateWithFinalizer(
		st,
		c,
		func(pid int, start uint64) (bool, error) { return false, nil },
		func(*state.Store, *state.Container) error {
			cleanupCalls++
			return nil
		},
		func(st *state.Store, dead *state.Container, exitCode int, finishedAt time.Time) (bool, error) {
			changed, err := st.MarkStoppedIfIdentity(dead.ID, dead.PID, dead.PIDStartTime, exitCode, finishedAt)
			if err != nil {
				t.Fatalf("commit stopped state: %v", err)
			}
			if !changed {
				t.Fatal("expected exact dead generation to transition to stopped")
			}
			return true, sentinel
		},
		func() time.Time { return time.Unix(500, 0) },
	)
	if !errors.Is(err, sentinel) {
		t.Fatalf("error=%v, want finalizer post-commit error", err)
	}
	if got == nil || got.Status != state.StatusStopped || got.PID != 0 || got.PIDStartTime != 0 || got.ExitCode != -1 {
		t.Fatalf("snapshot after post-commit error=%+v, want durable stopped state", got)
	}
	if cleanupCalls != 0 {
		t.Fatalf("generic stopped cleanup calls=%d, want 0; dead-generation finalizer owns cleanup", cleanupCalls)
	}
}

func TestReconcileDeadGenerationFinalizerCannotCleanupConcurrentRestartViaStoppedFallback(t *testing.T) {
	st, err := state.Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	c := saveReconcileContainer(t, st, &state.Container{
		ID:           "finalize-restart-race",
		Status:       state.StatusRunning,
		PID:          111,
		PIDStartTime: 11,
	})
	cleanupCalls := 0

	got, err := reconcileContainerStateWithFinalizer(
		st,
		c,
		func(pid int, start uint64) (bool, error) { return false, nil },
		func(*state.Store, *state.Container) error {
			cleanupCalls++
			return nil
		},
		func(st *state.Store, dead *state.Container, exitCode int, finishedAt time.Time) (bool, error) {
			changed, err := st.MarkStoppedIfIdentity(dead.ID, dead.PID, dead.PIDStartTime, exitCode, finishedAt)
			if err != nil {
				return changed, err
			}
			if err := st.MarkRunning(dead.ID, 222, 22, finishedAt.Add(time.Second)); err != nil {
				t.Fatalf("install concurrent restart: %v", err)
			}
			return changed, nil
		},
		func() time.Time { return time.Unix(600, 0) },
	)
	if err != nil {
		t.Fatalf("reconcile: %v", err)
	}
	if got.Status != state.StatusRunning || got.PID != 222 || got.PIDStartTime != 22 {
		t.Fatalf("snapshot=%+v, want concurrent running generation 222/22", got)
	}
	if cleanupCalls != 0 {
		t.Fatalf("generic stopped cleanup calls=%d, want 0 after concurrent restart", cleanupCalls)
	}
}

func TestReconcileFailsClosedWithoutDeadGenerationFinalizer(t *testing.T) {
	st, err := state.Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	c := saveReconcileContainer(t, st, &state.Container{ID: "nil-finalizer", Status: state.StatusRunning, PID: 7, PIDStartTime: 8})

	got, err := reconcileContainerStateWithFinalizer(
		st,
		c,
		func(pid int, start uint64) (bool, error) { return false, nil },
		func(*state.Store, *state.Container) error { return nil },
		nil,
		time.Now,
	)
	if err == nil {
		t.Fatal("expected nil finalizer to fail closed")
	}
	if got == nil || got.Status != state.StatusRunning || got.PID != 7 || got.PIDStartTime != 8 {
		t.Fatalf("snapshot after nil-finalizer rejection=%+v", got)
	}
}
