//go:build linux

package container

import (
	"os/exec"
	"testing"
	"time"

	"minicontainer/internal/state"
)

func TestWaitFallbackDoesNotPermanentlyLoseLateAuthoritativeExitCode(t *testing.T) {
	cmd := exec.Command("sh", "-c", "sleep 0.1; exit 23")
	if err := cmd.Start(); err != nil {
		t.Fatalf("start child: %v", err)
	}
	start, err := ProcessStartTime(cmd.Process.Pid)
	if err != nil {
		_ = cmd.Process.Kill()
		_ = cmd.Wait()
		t.Fatalf("ProcessStartTime: %v", err)
	}

	st, err := state.Open(t.TempDir())
	if err != nil {
		_ = cmd.Process.Kill()
		_ = cmd.Wait()
		t.Fatal(err)
	}
	const id = "ctr-late-authoritative-exit"
	if err := st.Save(&state.Container{
		ID:           id,
		Status:       state.StatusRunning,
		PID:          cmd.Process.Pid,
		PIDStartTime: start,
		CreatedAt:    time.Now(),
	}); err != nil {
		_ = cmd.Process.Kill()
		_ = cmd.Wait()
		t.Fatal(err)
	}

	fallbackCode, err := WaitContainer(st, id)
	if err != nil {
		_ = cmd.Wait()
		t.Fatalf("WaitContainer: %v", err)
	}
	if fallbackCode != -1 {
		_ = cmd.Wait()
		t.Fatalf("fallback exit code=%d, want unknown -1", fallbackCode)
	}

	waitErr := cmd.Wait()
	actualCode := exitCodeFromWaitError(waitErr)
	if actualCode != 23 {
		t.Fatalf("child exit code=%d, want 23 (wait err=%v)", actualCode, waitErr)
	}
	changed, err := st.MarkStoppedIfIdentity(id, cmd.Process.Pid, start, actualCode, time.Now())
	if err != nil {
		t.Fatalf("late authoritative MarkStoppedIfIdentity: %v", err)
	}
	if !changed {
		t.Fatal("late authoritative reaper could not upgrade unknown exit code")
	}

	rec, err := st.Get(id)
	if err != nil {
		t.Fatal(err)
	}
	if rec.Status != state.StatusStopped || rec.ExitCode != 23 {
		t.Fatalf("late authoritative exit code was lost: %+v", rec)
	}
}
