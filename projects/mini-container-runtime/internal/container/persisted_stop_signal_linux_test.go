//go:build linux

package container

import (
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

func TestPersistedStopSignalHelperProcess(t *testing.T) {
	if os.Getenv("MINICONTAINER_PERSISTED_STOP_SIGNAL_HELPER") != "1" {
		return
	}

	signal.Ignore(syscall.SIGTERM)
	ch := make(chan os.Signal, 1)
	signal.Notify(ch, syscall.SIGUSR1)
	defer signal.Stop(ch)

	fmt.Println("ready")
	select {
	case <-ch:
		marker := os.Getenv("MINICONTAINER_PERSISTED_STOP_SIGNAL_MARKER")
		if marker == "" {
			os.Exit(2)
		}
		if err := os.WriteFile(marker, []byte("SIGUSR1"), 0o600); err != nil {
			os.Exit(3)
		}
		os.Exit(0)
	case <-time.After(10 * time.Second):
		os.Exit(4)
	}
}

func TestStopContainerHonorsPersistedStopSignal(t *testing.T) {
	marker := filepath.Join(t.TempDir(), "signal-marker")
	cmd := exec.Command(os.Args[0], "-test.run=^TestPersistedStopSignalHelperProcess$")
	cmd.Env = append(os.Environ(),
		"MINICONTAINER_PERSISTED_STOP_SIGNAL_HELPER=1",
		"MINICONTAINER_PERSISTED_STOP_SIGNAL_MARKER="+marker,
	)
	stdout, err := cmd.StdoutPipe()
	if err != nil {
		t.Fatalf("helper stdout pipe: %v", err)
	}
	cmd.Stderr = os.Stderr
	if err := cmd.Start(); err != nil {
		t.Fatalf("start helper: %v", err)
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
	defer st.Close()

	const id = "ctr-persisted-stop-signal"
	if err := st.Save(&state.Container{
		ID:           id,
		Status:       state.StatusRunning,
		PID:          cmd.Process.Pid,
		PIDStartTime: start,
		CreatedAt:    time.Now(),
	}); err != nil {
		t.Fatalf("save running container: %v", err)
	}
	if err := st.SaveContainerStopSignal(id, "SIGUSR1"); err != nil {
		t.Fatalf("persist stop signal: %v", err)
	}

	if _, err := StopContainer(st, id, time.Second); err != nil {
		t.Fatalf("StopContainer: %v", err)
	}
	if IsRunning(cmd.Process.Pid) {
		t.Fatalf("process %d still running after persisted stop signal", cmd.Process.Pid)
	}
	got, err := os.ReadFile(marker)
	if err != nil {
		t.Fatalf("persisted signal marker missing: %v", err)
	}
	if string(got) != "SIGUSR1" {
		t.Fatalf("persisted signal marker=%q, want SIGUSR1", got)
	}
}
