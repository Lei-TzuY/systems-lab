package container

import (
	"errors"
	"testing"
	"time"

	"minicontainer/internal/state"
)

func saveReconcileContainer(t *testing.T, st *state.Store, c *state.Container) *state.Container {
	t.Helper()
	if err := st.Save(c); err != nil {
		t.Fatalf("save container: %v", err)
	}
	got, err := st.Get(c.ID)
	if err != nil {
		t.Fatalf("reload container: %v", err)
	}
	return got
}

func TestReconcileContainerStateKeepsExactLiveGenerationRunning(t *testing.T) {
	st, err := state.Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	c := saveReconcileContainer(t, st, &state.Container{
		ID:           "live-generation",
		Status:       state.StatusRunning,
		PID:          1234,
		PIDStartTime: 55,
	})

	cleanupCalls := 0
	got, err := reconcileContainerStateWith(
		st,
		c,
		func(pid int, start uint64) (bool, error) {
			if pid != 1234 || start != 55 {
				t.Fatalf("probe identity=%d/%d, want 1234/55", pid, start)
			}
			return true, nil
		},
		func(*state.Store, *state.Container) error {
			cleanupCalls++
			return nil
		},
		time.Now,
	)
	if err != nil {
		t.Fatalf("reconcile: %v", err)
	}
	if got.Status != state.StatusRunning || got.PID != 1234 || got.PIDStartTime != 55 {
		t.Fatalf("reconciled state=%+v, want original running generation", got)
	}
	if cleanupCalls != 0 {
		t.Fatalf("cleanup calls=%d, want 0 for live generation", cleanupCalls)
	}
}

func TestReconcileContainerStateStopsGoneOrReusedGeneration(t *testing.T) {
	st, err := state.Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	c := saveReconcileContainer(t, st, &state.Container{
		ID:           "stale-generation",
		Status:       state.StatusRunning,
		PID:          4321,
		PIDStartTime: 77,
	})

	cleanupCalls := 0
	got, err := reconcileContainerStateWith(
		st,
		c,
		func(pid int, start uint64) (bool, error) { return false, nil },
		func(_ *state.Store, stopped *state.Container) error {
			cleanupCalls++
			if stopped.Status != state.StatusStopped {
				t.Fatalf("cleanup saw status %s, want stopped", stopped.Status)
			}
			return nil
		},
		func() time.Time { return time.Unix(100, 0) },
	)
	if err != nil {
		t.Fatalf("reconcile: %v", err)
	}
	if got.Status != state.StatusStopped || got.PID != 0 || got.PIDStartTime != 0 || got.ExitCode != -1 {
		t.Fatalf("reconciled state=%+v, want stopped unknown-exit generation", got)
	}
	if cleanupCalls != 1 {
		t.Fatalf("cleanup calls=%d, want 1", cleanupCalls)
	}
}

func TestReconcileContainerStateCannotClobberConcurrentRestart(t *testing.T) {
	st, err := state.Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	c := saveReconcileContainer(t, st, &state.Container{
		ID:           "restart-race",
		Status:       state.StatusRunning,
		PID:          1111,
		PIDStartTime: 10,
	})

	cleanupCalls := 0
	got, err := reconcileContainerStateWith(
		st,
		c,
		func(pid int, start uint64) (bool, error) {
			if pid != 1111 || start != 10 {
				t.Fatalf("probe identity=%d/%d, want old generation 1111/10", pid, start)
			}
			if _, err := st.MarkStoppedIfIdentity(c.ID, 1111, 10, -1, time.Unix(200, 0)); err != nil {
				t.Fatalf("concurrent stop: %v", err)
			}
			if err := st.MarkRunning(c.ID, 2222, 20, time.Unix(201, 0)); err != nil {
				t.Fatalf("concurrent restart: %v", err)
			}
			return false, nil
		},
		func(*state.Store, *state.Container) error {
			cleanupCalls++
			return nil
		},
		time.Now,
	)
	if err != nil {
		t.Fatalf("reconcile: %v", err)
	}
	if got.Status != state.StatusRunning || got.PID != 2222 || got.PIDStartTime != 20 {
		t.Fatalf("reconciled state=%+v, want concurrent generation 2222/20", got)
	}
	if cleanupCalls != 0 {
		t.Fatalf("cleanup calls=%d, want 0 for concurrent running generation", cleanupCalls)
	}
}

func TestReconcileContainerStateRetriesStoppedCleanup(t *testing.T) {
	st, err := state.Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	c := saveReconcileContainer(t, st, &state.Container{ID: "stopped-cleanup", Status: state.StatusStopped})

	probeCalls := 0
	cleanupCalls := 0
	got, err := reconcileContainerStateWith(
		st,
		c,
		func(pid int, start uint64) (bool, error) {
			probeCalls++
			return false, nil
		},
		func(_ *state.Store, stopped *state.Container) error {
			cleanupCalls++
			if stopped.ID != c.ID || stopped.Status != state.StatusStopped {
				t.Fatalf("cleanup snapshot=%+v", stopped)
			}
			return nil
		},
		time.Now,
	)
	if err != nil {
		t.Fatalf("reconcile: %v", err)
	}
	if got.Status != state.StatusStopped {
		t.Fatalf("status=%s, want stopped", got.Status)
	}
	if probeCalls != 0 {
		t.Fatalf("probe calls=%d, want 0 for stopped state", probeCalls)
	}
	if cleanupCalls != 1 {
		t.Fatalf("cleanup calls=%d, want 1", cleanupCalls)
	}
}

func TestReconcileContainerStateFailsClosedWithoutGenerationIdentity(t *testing.T) {
	st, err := state.Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	c := saveReconcileContainer(t, st, &state.Container{
		ID:     "legacy-running",
		Status: state.StatusRunning,
		PID:    9876,
	})

	probeCalls := 0
	got, err := reconcileContainerStateWith(
		st,
		c,
		func(pid int, start uint64) (bool, error) {
			probeCalls++
			return false, nil
		},
		func(*state.Store, *state.Container) error { return nil },
		time.Now,
	)
	if err == nil || !errors.Is(err, ErrProcessIdentityUnavailable) {
		t.Fatalf("error=%v, want ErrProcessIdentityUnavailable", err)
	}
	if got == nil || got.Status != state.StatusRunning || got.PID != 9876 {
		t.Fatalf("state after failed-closed reconcile=%+v", got)
	}
	if probeCalls != 0 {
		t.Fatalf("probe calls=%d, want 0 without full identity", probeCalls)
	}
}

func TestReconcileContainerStateSurfacesProbeFailureWithoutMutation(t *testing.T) {
	st, err := state.Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	c := saveReconcileContainer(t, st, &state.Container{
		ID:           "probe-error",
		Status:       state.StatusRunning,
		PID:          2468,
		PIDStartTime: 99,
	})
	sentinel := errors.New("pidfd poll failed")

	got, err := reconcileContainerStateWith(
		st,
		c,
		func(pid int, start uint64) (bool, error) { return false, sentinel },
		func(*state.Store, *state.Container) error { return nil },
		time.Now,
	)
	if !errors.Is(err, sentinel) {
		t.Fatalf("error=%v, want sentinel probe failure", err)
	}
	if got == nil || got.Status != state.StatusRunning || got.PID != 2468 || got.PIDStartTime != 99 {
		t.Fatalf("state after probe failure=%+v", got)
	}
}
