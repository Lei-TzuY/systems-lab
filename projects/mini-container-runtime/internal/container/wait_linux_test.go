//go:build linux

package container

import (
	"errors"
	"os/exec"
	"testing"
	"time"

	"minicontainer/internal/state"
)

func TestWaitContainerReconcilesExitedRunningState(t *testing.T) {
	cmd := exec.Command("true")
	if err := cmd.Start(); err != nil {
		t.Fatal(err)
	}
	start, err := ProcessStartTime(cmd.Process.Pid)
	if err != nil {
		t.Fatalf("ProcessStartTime: %v", err)
	}
	if err := cmd.Wait(); err != nil {
		t.Fatalf("wait child: %v", err)
	}

	st, err := state.Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	if err := st.Save(&state.Container{
		ID:           "ctr-wait-exited",
		Status:       state.StatusRunning,
		PID:          cmd.Process.Pid,
		PIDStartTime: start,
		CreatedAt:    time.Now(),
	}); err != nil {
		t.Fatal(err)
	}

	code, err := WaitContainer(st, "ctr-wait-exited")
	if err != nil {
		t.Fatalf("WaitContainer: %v", err)
	}
	if code != -1 {
		t.Fatalf("exit code=%d, want unknown -1", code)
	}
	rec, err := st.Get("ctr-wait-exited")
	if err != nil {
		t.Fatal(err)
	}
	if rec.Status != state.StatusStopped || rec.PID != 0 || rec.PIDStartTime != 0 {
		t.Fatalf("stale running state not reconciled: %+v", rec)
	}
}

func TestWaitContainerRejectsMissingRunningIdentity(t *testing.T) {
	st, err := state.Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	if err := st.Save(&state.Container{
		ID:        "ctr-wait-legacy",
		Status:    state.StatusRunning,
		PID:       1234,
		CreatedAt: time.Now(),
	}); err != nil {
		t.Fatal(err)
	}

	_, err = WaitContainer(st, "ctr-wait-legacy")
	if !errors.Is(err, ErrProcessIdentityUnavailable) {
		t.Fatalf("expected missing identity error, got %v", err)
	}
}

func TestWaitContainerReturnsPersistedExitCodeFromConcurrentLifecycleWriter(t *testing.T) {
	cmd := exec.Command("sleep", "30")
	if err := cmd.Start(); err != nil {
		t.Fatal(err)
	}
	defer func() {
		if cmd.Process != nil && IsRunning(cmd.Process.Pid) {
			_ = cmd.Process.Kill()
		}
		_ = cmd.Wait()
	}()
	start, err := ProcessStartTime(cmd.Process.Pid)
	if err != nil {
		t.Fatal(err)
	}
	st, err := state.Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	if err := st.Save(&state.Container{
		ID:           "ctr-wait-writer",
		Status:       state.StatusRunning,
		PID:          cmd.Process.Pid,
		PIDStartTime: start,
		CreatedAt:    time.Now(),
	}); err != nil {
		t.Fatal(err)
	}

	done := make(chan struct{})
	go func() {
		defer close(done)
		time.Sleep(50 * time.Millisecond)
		_, _ = st.MarkStoppedIfIdentity("ctr-wait-writer", cmd.Process.Pid, start, 23, time.Now())
	}()

	code, err := WaitContainer(st, "ctr-wait-writer")
	<-done
	if err != nil {
		t.Fatalf("WaitContainer: %v", err)
	}
	if code != 23 {
		t.Fatalf("exit code=%d, want persisted 23", code)
	}
}
