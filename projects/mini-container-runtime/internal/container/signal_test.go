//go:build linux

package container

import (
	"errors"
	"os/exec"
	"testing"
	"time"

	"minicontainer/internal/state"
)

func TestSendSignalUsesPersistedProcessIdentity(t *testing.T) {
	cmd := exec.Command("sleep", "30")
	if err := cmd.Start(); err != nil {
		t.Fatalf("start child process: %v", err)
	}
	defer func() {
		if cmd.Process != nil && IsRunning(cmd.Process.Pid) {
			_ = cmd.Process.Kill()
		}
		_ = cmd.Wait()
	}()

	start, err := ProcessStartTime(cmd.Process.Pid)
	if err != nil {
		t.Fatalf("ProcessStartTime: %v", err)
	}
	st, err := state.Open(t.TempDir())
	if err != nil {
		t.Fatalf("Open state store error: %v", err)
	}

	c := &state.Container{
		ID:           "ctr-sig-1",
		Status:       state.StatusRunning,
		PID:          cmd.Process.Pid,
		PIDStartTime: start,
		RootFS:       "/rootfs-signal-proof",
		CreatedAt:    time.Now(),
	}
	if err := st.Save(c); err != nil {
		t.Fatalf("save container state: %v", err)
	}

	// SIGCONT is observable through the pidfd send path but does not terminate
	// an ordinary sleeping process, keeping the test deterministic.
	resolved, err := SendSignalResolved(st, "ctr-sig", "SIGCONT")
	if err != nil {
		t.Fatalf("SendSignalResolved error: %v", err)
	}
	if resolved.ID != c.ID || resolved.PID != c.PID || resolved.PIDStartTime != start || resolved.RootFS != c.RootFS {
		t.Fatalf("resolved snapshot=%+v, want exact persisted signal target %+v", resolved, c)
	}
	// Preserve the original API contract for callers that only need an error.
	if err := SendSignal(st, c.ID, "SIGCONT"); err != nil {
		t.Fatalf("SendSignal wrapper error: %v", err)
	}
	ok, err := ProcessIdentityMatches(c.PID, start)
	if err != nil || !ok {
		t.Fatalf("process identity after signal: match=%v err=%v", ok, err)
	}
}

func TestSendSignalRejectsStaleIdentity(t *testing.T) {
	cmd := exec.Command("sleep", "30")
	if err := cmd.Start(); err != nil {
		t.Fatal(err)
	}
	defer func() {
		_ = cmd.Process.Kill()
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
		ID:           "ctr-stale-signal",
		Status:       state.StatusRunning,
		PID:          cmd.Process.Pid,
		PIDStartTime: start + 1,
		CreatedAt:    time.Now(),
	}); err != nil {
		t.Fatal(err)
	}

	err = SendSignal(st, "ctr-stale-signal", "SIGKILL")
	if !errors.Is(err, ErrProcessIdentityMismatch) {
		t.Fatalf("expected identity mismatch, got %v", err)
	}
	ok, err := ProcessIdentityMatches(cmd.Process.Pid, start)
	if err != nil || !ok {
		t.Fatalf("stale SendSignal affected unrelated process: match=%v err=%v", ok, err)
	}
}

func TestSendSignalRejectsMissingIdentity(t *testing.T) {
	st, err := state.Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	if err := st.Save(&state.Container{
		ID:        "ctr-legacy-signal",
		Status:    state.StatusRunning,
		PID:       1234,
		CreatedAt: time.Now(),
	}); err != nil {
		t.Fatal(err)
	}
	if err := SendSignal(st, "ctr-legacy-signal", "SIGTERM"); !errors.Is(err, ErrProcessIdentityUnavailable) {
		t.Fatalf("expected missing identity error, got %v", err)
	}
}
