//go:build linux

package container

import (
	"errors"
	"fmt"
	"os"
	"os/exec"
	"os/signal"
	"path/filepath"
	"syscall"
	"testing"
	"time"

	"minicontainer/internal/state"
)

func saveRunningTestContainer(t *testing.T, st *state.Store, id string, cmd *exec.Cmd, start uint64) {
	t.Helper()
	if err := st.Save(&state.Container{
		ID:           id,
		Status:       state.StatusRunning,
		PID:          cmd.Process.Pid,
		PIDStartTime: start,
		CreatedAt:    time.Now(),
	}); err != nil {
		t.Fatalf("save running container: %v", err)
	}
}

func TestStopContainerWithSignalHelperProcess(t *testing.T) {
	if os.Getenv("MINICONTAINER_STOP_SIGNAL_HELPER") != "1" {
		return
	}

	// A hard-coded SIGTERM implementation must not accidentally satisfy the
	// parent regression. Only SIGUSR1 records success and exits voluntarily.
	signal.Ignore(syscall.SIGTERM)
	ch := make(chan os.Signal, 1)
	signal.Notify(ch, syscall.SIGUSR1)
	defer signal.Stop(ch)

	fmt.Println("ready")
	select {
	case got := <-ch:
		if got != syscall.SIGUSR1 {
			os.Exit(2)
		}
		marker := os.Getenv("MINICONTAINER_STOP_SIGNAL_MARKER")
		if marker == "" {
			os.Exit(3)
		}
		if err := os.WriteFile(marker, []byte("SIGUSR1"), 0o600); err != nil {
			os.Exit(4)
		}
		os.Exit(0)
	case <-time.After(10 * time.Second):
		os.Exit(5)
	}
}

func TestStopContainerTerminatesExactProcessAndReconcilesState(t *testing.T) {
	cmd := exec.Command("sleep", "30")
	if err := cmd.Start(); err != nil {
		t.Fatalf("start child: %v", err)
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
		t.Fatalf("open state: %v", err)
	}
	saveRunningTestContainer(t, st, "ctr-stop-exact", cmd, start)

	if _, err := StopContainer(st, "ctr-stop-exact", time.Second); err != nil {
		t.Fatalf("StopContainer: %v", err)
	}
	if IsRunning(cmd.Process.Pid) {
		t.Fatalf("process %d still running after StopContainer", cmd.Process.Pid)
	}

	rec, err := st.Get("ctr-stop-exact")
	if err != nil {
		t.Fatalf("reload state: %v", err)
	}
	if rec.Status != state.StatusStopped || rec.PID != 0 || rec.PIDStartTime != 0 {
		t.Fatalf("state not reconciled: status=%s pid=%d start=%d", rec.Status, rec.PID, rec.PIDStartTime)
	}
	if rec.FinishedAt == nil || rec.ExitCode != -1 {
		t.Fatalf("stop metadata not recorded: finished=%v exit=%d", rec.FinishedAt, rec.ExitCode)
	}
}

func TestStopContainerWithSignalDeliversConfiguredGracefulSignal(t *testing.T) {
	marker := filepath.Join(t.TempDir(), "signal-marker")
	cmd := exec.Command(os.Args[0], "-test.run=^TestStopContainerWithSignalHelperProcess$")
	cmd.Env = append(os.Environ(),
		"MINICONTAINER_STOP_SIGNAL_HELPER=1",
		"MINICONTAINER_STOP_SIGNAL_MARKER="+marker,
	)
	stdout, err := cmd.StdoutPipe()
	if err != nil {
		t.Fatalf("helper stdout pipe: %v", err)
	}
	cmd.Stderr = os.Stderr
	if err := cmd.Start(); err != nil {
		t.Fatalf("start stop-signal helper: %v", err)
	}
	defer func() {
		if cmd.Process != nil && IsRunning(cmd.Process.Pid) {
			_ = cmd.Process.Kill()
		}
		_ = cmd.Wait()
	}()

	var ready string
	if _, err := fmt.Fscan(stdout, &ready); err != nil {
		t.Fatalf("read helper readiness: %v", err)
	}
	if ready != "ready" {
		t.Fatalf("helper readiness=%q, want ready", ready)
	}

	start, err := ProcessStartTime(cmd.Process.Pid)
	if err != nil {
		t.Fatalf("ProcessStartTime: %v", err)
	}
	st, err := state.Open(t.TempDir())
	if err != nil {
		t.Fatalf("open state: %v", err)
	}
	saveRunningTestContainer(t, st, "ctr-stop-sigusr1", cmd, start)

	if _, err := StopContainerWithSignal(st, "ctr-stop-sigusr1", "SIGUSR1", time.Second); err != nil {
		t.Fatalf("StopContainerWithSignal: %v", err)
	}
	if IsRunning(cmd.Process.Pid) {
		t.Fatalf("process %d still running after configured SIGUSR1", cmd.Process.Pid)
	}
	got, err := os.ReadFile(marker)
	if err != nil {
		t.Fatalf("configured signal marker missing: %v", err)
	}
	if string(got) != "SIGUSR1" {
		t.Fatalf("configured signal marker=%q, want SIGUSR1", got)
	}
}

func TestStopContainerWithSignalRejectsInvalidSignalBeforeSignaling(t *testing.T) {
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
	saveRunningTestContainer(t, st, "ctr-stop-invalid-signal", cmd, start)

	if _, err := StopContainerWithSignal(st, "ctr-stop-invalid-signal", "NOTASIGNAL", time.Second); err == nil {
		t.Fatal("StopContainerWithSignal accepted invalid signal")
	}
	match, err := ProcessIdentityMatches(cmd.Process.Pid, start)
	if err != nil || !match {
		t.Fatalf("invalid stop signal affected live process: match=%v err=%v", match, err)
	}
}

func TestStopContainerRejectsStaleIdentityWithoutSignaling(t *testing.T) {
	cmd := exec.Command("sleep", "30")
	if err := cmd.Start(); err != nil {
		t.Fatalf("start child: %v", err)
	}
	defer func() {
		_ = cmd.Process.Kill()
		_ = cmd.Wait()
	}()

	start, err := ProcessStartTime(cmd.Process.Pid)
	if err != nil {
		t.Fatalf("ProcessStartTime: %v", err)
	}
	st, err := state.Open(t.TempDir())
	if err != nil {
		t.Fatalf("open state: %v", err)
	}
	saveRunningTestContainer(t, st, "ctr-stop-stale", cmd, start+1)

	_, err = StopContainer(st, "ctr-stop-stale", 10*time.Millisecond)
	if !errors.Is(err, ErrProcessIdentityMismatch) {
		t.Fatalf("expected identity mismatch, got %v", err)
	}
	match, err := ProcessIdentityMatches(cmd.Process.Pid, start)
	if err != nil || !match {
		t.Fatalf("stale stop affected live process: match=%v err=%v", match, err)
	}
}

func TestStopContainerRejectsMissingIdentity(t *testing.T) {
	st, err := state.Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	if err := st.Save(&state.Container{
		ID:        "ctr-stop-legacy",
		Status:    state.StatusRunning,
		PID:       1234,
		CreatedAt: time.Now(),
	}); err != nil {
		t.Fatal(err)
	}
	if _, err := StopContainer(st, "ctr-stop-legacy", time.Second); !errors.Is(err, ErrProcessIdentityUnavailable) {
		t.Fatalf("expected missing identity error, got %v", err)
	}
}

func TestSendSignalLeavesLifecycleStateRunning(t *testing.T) {
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
	saveRunningTestContainer(t, st, "ctr-signal-state", cmd, start)

	if err := SendSignal(st, "ctr-signal-state", "SIGCONT"); err != nil {
		t.Fatalf("SendSignal: %v", err)
	}
	rec, err := st.Get("ctr-signal-state")
	if err != nil {
		t.Fatal(err)
	}
	if rec.Status != state.StatusRunning || rec.PID != cmd.Process.Pid || rec.PIDStartTime != start {
		t.Fatalf("non-terminating signal mutated lifecycle state: %+v", rec)
	}
}
