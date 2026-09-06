//go:build linux

package container

import (
	"errors"
	"os/exec"
	"syscall"
	"testing"
	"time"
)

func TestProcessHandleSignalsExactProcessAndWaitsForExit(t *testing.T) {
	cmd := exec.Command("sleep", "30")
	if err := cmd.Start(); err != nil {
		t.Fatal(err)
	}
	defer func() {
		if IsRunning(cmd.Process.Pid) {
			_ = cmd.Process.Kill()
		}
		_ = cmd.Wait()
	}()

	start, err := ProcessStartTime(cmd.Process.Pid)
	if err != nil {
		t.Fatal(err)
	}
	h, err := OpenProcessHandle(cmd.Process.Pid, start)
	if err != nil {
		t.Fatalf("OpenProcessHandle: %v", err)
	}
	defer h.Close()

	exited, err := h.WaitExit(20 * time.Millisecond)
	if err != nil {
		t.Fatalf("WaitExit before signal: %v", err)
	}
	if exited {
		t.Fatal("live child unexpectedly reported exited")
	}
	if err := h.Signal(syscall.SIGTERM); err != nil {
		t.Fatalf("Signal: %v", err)
	}
	exited, err = h.WaitExit(time.Second)
	if err != nil {
		t.Fatalf("WaitExit after signal: %v", err)
	}
	if !exited {
		t.Fatal("pidfd did not report process exit")
	}
}

func TestOpenProcessHandleRejectsStaleStarttime(t *testing.T) {
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

	h, err := OpenProcessHandle(cmd.Process.Pid, start+1)
	if h != nil {
		_ = h.Close()
		t.Fatal("stale identity unexpectedly returned process handle")
	}
	if !errors.Is(err, ErrProcessIdentityMismatch) {
		t.Fatalf("expected identity mismatch, got %v", err)
	}
	ok, err := ProcessIdentityMatches(cmd.Process.Pid, start)
	if err != nil || !ok {
		t.Fatalf("unrelated process changed: match=%v err=%v", ok, err)
	}
}

func TestOpenProcessHandleRejectsInvalidIdentity(t *testing.T) {
	for _, tc := range []struct {
		pid   int
		start uint64
	}{
		{0, 1},
		{-1, 1},
		{1, 0},
	} {
		if _, err := OpenProcessHandle(tc.pid, tc.start); !errors.Is(err, ErrProcessIdentityUnavailable) {
			t.Fatalf("identity %d/%d error=%v", tc.pid, tc.start, err)
		}
	}
}

func TestProcessHandleCloseIsIdempotent(t *testing.T) {
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
	h, err := OpenProcessHandle(cmd.Process.Pid, start)
	if err != nil {
		t.Fatal(err)
	}
	if err := h.Close(); err != nil {
		t.Fatal(err)
	}
	if err := h.Close(); err != nil {
		t.Fatalf("second Close: %v", err)
	}
	if err := h.Signal(syscall.SIGCONT); err == nil {
		t.Fatal("closed process handle accepted signal")
	}
}
