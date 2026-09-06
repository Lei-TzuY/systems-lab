package state

import (
	"errors"
	"strings"
	"testing"
	"time"
)

func TestWithRunningGenerationLockedRunsOnlyForExactGeneration(t *testing.T) {
	st, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	defer st.Close()

	c := &Container{
		ID:           "generation-lock-test",
		Status:       StatusRunning,
		PID:          4321,
		PIDStartTime: 9876,
		CreatedAt:    time.Now(),
	}
	if err := st.Save(c); err != nil {
		t.Fatal(err)
	}

	cause := errors.New("callback failure")
	calls := 0
	err = st.WithRunningGenerationLocked(c.ID, c.PID, c.PIDStartTime, func() error {
		calls++
		return cause
	})
	if !errors.Is(err, cause) {
		t.Fatalf("error=%v, want callback cause", err)
	}
	if calls != 1 {
		t.Fatalf("callback calls=%d, want 1", calls)
	}
}

func TestWithRunningGenerationLockedRejectsChangedGeneration(t *testing.T) {
	for _, tc := range []struct {
		name  string
		state Container
		pid   int
		start uint64
	}{
		{
			name:  "stopped",
			state: Container{Status: StatusStopped, PID: 4321, PIDStartTime: 9876},
			pid:   4321,
			start: 9876,
		},
		{
			name:  "pid changed",
			state: Container{Status: StatusRunning, PID: 4322, PIDStartTime: 9876},
			pid:   4321,
			start: 9876,
		},
		{
			name:  "start time changed",
			state: Container{Status: StatusRunning, PID: 4321, PIDStartTime: 9877},
			pid:   4321,
			start: 9876,
		},
	} {
		t.Run(tc.name, func(t *testing.T) {
			st, err := Open(t.TempDir())
			if err != nil {
				t.Fatal(err)
			}
			defer st.Close()

			tc.state.ID = "generation-lock-test"
			tc.state.CreatedAt = time.Now()
			if err := st.Save(&tc.state); err != nil {
				t.Fatal(err)
			}

			called := false
			err = st.WithRunningGenerationLocked(tc.state.ID, tc.pid, tc.start, func() error {
				called = true
				return nil
			})
			if err == nil || !strings.Contains(err.Error(), "running generation changed") {
				t.Fatalf("error=%v, want generation-changed failure", err)
			}
			if called {
				t.Fatal("callback ran for changed generation")
			}
		})
	}
}

func TestWithRunningGenerationLockedRejectsInvalidArguments(t *testing.T) {
	st, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	defer st.Close()

	if err := st.WithRunningGenerationLocked("bad/id", 1, 1, func() error { return nil }); err == nil {
		t.Fatal("invalid id accepted")
	}
	if err := st.WithRunningGenerationLocked("valid-id", 0, 1, func() error { return nil }); err == nil {
		t.Fatal("invalid pid accepted")
	}
	if err := st.WithRunningGenerationLocked("valid-id", 1, 0, func() error { return nil }); err == nil {
		t.Fatal("missing start time accepted")
	}
	if err := st.WithRunningGenerationLocked("valid-id", 1, 1, nil); err == nil {
		t.Fatal("nil callback accepted")
	}
}

func TestWithRunningGenerationLockedRejectsClosedStore(t *testing.T) {
	st, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	if err := st.Close(); err != nil {
		t.Fatal(err)
	}

	called := false
	err = st.WithRunningGenerationLocked("closed-store-test", 1, 1, func() error {
		called = true
		return nil
	})
	if !errors.Is(err, ErrStoreClosed) {
		t.Fatalf("error=%v, want ErrStoreClosed", err)
	}
	if called {
		t.Fatal("callback ran after store close")
	}
}
