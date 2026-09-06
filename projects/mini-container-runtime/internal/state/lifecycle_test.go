package state

import (
	"testing"
	"time"
)

func TestLifecycleTransitionsBindProcessIdentity(t *testing.T) {
	st, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	created := time.Now().Add(-time.Second)
	if err := st.Save(&Container{ID: "ctr-1", Status: StatusCreated, CreatedAt: created}); err != nil {
		t.Fatal(err)
	}

	started := time.Now()
	if err := st.MarkRunning("ctr-1", 4242, 123456, started); err != nil {
		t.Fatalf("MarkRunning: %v", err)
	}
	running, err := st.Get("ctr-1")
	if err != nil {
		t.Fatal(err)
	}
	if running.Status != StatusRunning || running.PID != 4242 || running.PIDStartTime != 123456 {
		t.Fatalf("unexpected running state: %+v", running)
	}
	if running.StartedAt == nil || !running.StartedAt.Equal(started) || running.FinishedAt != nil {
		t.Fatalf("unexpected lifecycle timestamps: %+v", running)
	}

	finished := time.Now().Add(time.Second)
	changed, err := st.MarkStoppedIfIdentity("ctr-1", 4242, 123456, 17, finished)
	if err != nil {
		t.Fatalf("MarkStoppedIfIdentity: %v", err)
	}
	if !changed {
		t.Fatal("expected matching identity to transition state")
	}
	stopped, err := st.Get("ctr-1")
	if err != nil {
		t.Fatal(err)
	}
	if stopped.Status != StatusStopped || stopped.PID != 0 || stopped.PIDStartTime != 0 || stopped.ExitCode != 17 {
		t.Fatalf("unexpected stopped state: %+v", stopped)
	}
	if stopped.FinishedAt == nil || !stopped.FinishedAt.Equal(finished) {
		t.Fatalf("unexpected finish timestamp: %+v", stopped.FinishedAt)
	}
}

func TestMarkStoppedRejectsStaleIdentity(t *testing.T) {
	st, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	if err := st.Save(&Container{ID: "ctr-2", Status: StatusCreated, CreatedAt: time.Now()}); err != nil {
		t.Fatal(err)
	}
	if err := st.MarkRunning("ctr-2", 100, 200, time.Now()); err != nil {
		t.Fatal(err)
	}

	for _, tc := range []struct {
		name  string
		pid   int
		start uint64
	}{
		{"reused pid", 100, 201},
		{"different pid", 101, 200},
	} {
		t.Run(tc.name, func(t *testing.T) {
			changed, err := st.MarkStoppedIfIdentity("ctr-2", tc.pid, tc.start, 9, time.Now())
			if err != nil {
				t.Fatalf("unexpected error: %v", err)
			}
			if changed {
				t.Fatal("stale identity unexpectedly changed state")
			}
			current, err := st.Get("ctr-2")
			if err != nil {
				t.Fatal(err)
			}
			if current.Status != StatusRunning || current.PID != 100 || current.PIDStartTime != 200 {
				t.Fatalf("stale transition mutated state: %+v", current)
			}
		})
	}
}

func TestMarkRunningRejectsCompetingLiveIdentity(t *testing.T) {
	st, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	if err := st.Save(&Container{ID: "ctr-3", Status: StatusCreated, CreatedAt: time.Now()}); err != nil {
		t.Fatal(err)
	}
	if err := st.MarkRunning("ctr-3", 1, 10, time.Now()); err != nil {
		t.Fatal(err)
	}
	if err := st.MarkRunning("ctr-3", 2, 20, time.Now()); err == nil {
		t.Fatal("expected competing running identity to be rejected")
	}
}

func TestLifecycleValidation(t *testing.T) {
	st, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	if err := st.Save(&Container{ID: "ctr-4", Status: StatusCreated, CreatedAt: time.Now()}); err != nil {
		t.Fatal(err)
	}
	for _, tc := range []struct {
		pid   int
		start uint64
	}{
		{0, 1},
		{-1, 1},
		{1, 0},
	} {
		if err := st.MarkRunning("ctr-4", tc.pid, tc.start, time.Now()); err == nil {
			t.Fatalf("expected invalid running identity %d/%d to fail", tc.pid, tc.start)
		}
	}
}
