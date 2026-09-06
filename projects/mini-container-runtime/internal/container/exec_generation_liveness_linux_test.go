//go:build linux

package container

import (
	"os/exec"
	"strings"
	"testing"
	"time"
)

func TestRequireExecTargetAliveRejectsExitedGeneration(t *testing.T) {
	cmd := exec.Command("sh", "-c", "sleep 30")
	if err := cmd.Start(); err != nil {
		t.Fatalf("start target process: %v", err)
	}
	defer func() {
		_ = cmd.Process.Kill()
		_ = cmd.Wait()
	}()

	startTime, err := ProcessStartTime(cmd.Process.Pid)
	if err != nil {
		t.Fatalf("target start time: %v", err)
	}
	handle, err := OpenProcessHandle(cmd.Process.Pid, startTime)
	if err != nil {
		t.Fatalf("open process handle: %v", err)
	}
	targets := &execTargets{rootFD: -1, startTime: startTime, process: handle}
	defer targets.close()

	if err := requireExecTargetAlive(targets, "before test exit"); err != nil {
		t.Fatalf("live generation rejected: %v", err)
	}
	if err := cmd.Process.Kill(); err != nil {
		t.Fatalf("kill target process: %v", err)
	}

	deadline := time.Now().Add(2 * time.Second)
	for {
		exited, err := handle.WaitExit(0)
		if err != nil {
			t.Fatalf("poll target exit: %v", err)
		}
		if exited {
			break
		}
		if time.Now().After(deadline) {
			t.Fatal("target process did not become pidfd-readable after exit")
		}
		time.Sleep(5 * time.Millisecond)
	}

	err = requireExecTargetAlive(targets, "before payload spawn")
	if err == nil || !strings.Contains(err.Error(), "exited before payload spawn") {
		t.Fatalf("exited generation accepted: %v", err)
	}
}

func TestRequireExecTargetAliveRejectsMissingHandle(t *testing.T) {
	if err := requireExecTargetAlive(nil, "during test"); err == nil {
		t.Fatal("nil exec target accepted")
	}
	if err := requireExecTargetAlive(&execTargets{rootFD: -1}, "during test"); err == nil {
		t.Fatal("exec target without process handle accepted")
	}
}
