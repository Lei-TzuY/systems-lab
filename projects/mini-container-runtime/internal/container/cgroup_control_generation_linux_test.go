//go:build linux

package container

import (
	"errors"
	"os/exec"
	"testing"
	"time"

	"minicontainer/internal/cgroups"
	"minicontainer/internal/state"
)

func TestControlRunningCgroupReturnsExactLiveGeneration(t *testing.T) {
	cmd, start := startCgroupControlTestProcess(t)
	defer cleanupCgroupControlTestProcess(cmd)

	st, err := state.Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	const id = "ctr-control-live-abcdef"
	rootfs := "/rootfs-control-proof"
	saveCgroupControlTestState(t, st, id, cmd.Process.Pid, start, rootfs)

	wantName, err := cgroups.NameForContainerProcess(id, cmd.Process.Pid, start)
	if err != nil {
		t.Fatal(err)
	}
	called := 0
	resolved, err := controlRunningCgroup(st, "ctr-control-live", "test", func(name string) error {
		called++
		if name != wantName {
			t.Fatalf("cgroup name=%q, want %q", name, wantName)
		}
		return nil
	})
	if err != nil {
		t.Fatalf("controlRunningCgroup: %v", err)
	}
	if called != 1 {
		t.Fatalf("control callback calls=%d, want 1", called)
	}
	if resolved.ID != id || resolved.PID != cmd.Process.Pid || resolved.PIDStartTime != start || resolved.RootFS != rootfs {
		t.Fatalf("resolved=%+v, want exact generation id=%s pid=%d start=%d rootfs=%s", resolved, id, cmd.Process.Pid, start, rootfs)
	}
}

func TestControlRunningCgroupRejectsGenerationThatExitedDuringControl(t *testing.T) {
	cmd, start := startCgroupControlTestProcess(t)

	st, err := state.Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	const id = "ctr-control-exit"
	saveCgroupControlTestState(t, st, id, cmd.Process.Pid, start, "/rootfs")

	resolved, err := controlRunningCgroup(st, id, "test", func(string) error {
		if err := cmd.Process.Kill(); err != nil {
			t.Fatalf("kill child during control: %v", err)
		}
		if err := cmd.Wait(); err == nil {
			t.Fatal("SIGKILL child unexpectedly exited without wait error")
		}
		return nil
	})
	if resolved != nil {
		t.Fatalf("resolved=%+v, want nil after generation exited during control", resolved)
	}
	if !errors.Is(err, ErrProcessNotFound) {
		t.Fatalf("error=%v, want ErrProcessNotFound", err)
	}
}

func TestControlRunningCgroupRejectsExitedZombieBeforeControl(t *testing.T) {
	cmd, start := startCgroupControlTestProcess(t)
	defer cleanupCgroupControlTestProcess(cmd)

	st, err := state.Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	const id = "ctr-control-zombie"
	saveCgroupControlTestState(t, st, id, cmd.Process.Pid, start, "/rootfs")

	proof, err := OpenProcessHandle(cmd.Process.Pid, start)
	if err != nil {
		t.Fatalf("OpenProcessHandle: %v", err)
	}
	defer proof.Close()
	if err := cmd.Process.Kill(); err != nil {
		t.Fatal(err)
	}
	exited, err := proof.WaitExit(time.Second)
	if err != nil || !exited {
		t.Fatalf("wait for unreaped child exit: exited=%v err=%v", exited, err)
	}

	called := false
	_, err = controlRunningCgroup(st, id, "test", func(string) error {
		called = true
		return nil
	})
	if called {
		t.Fatal("control callback ran for an exited unreaped generation")
	}
	if !errors.Is(err, ErrProcessNotFound) {
		t.Fatalf("error=%v, want ErrProcessNotFound", err)
	}
}

func TestControlRunningCgroupRejectsAlreadyReapedGenerationBeforeControl(t *testing.T) {
	cmd, start := startCgroupControlTestProcess(t)
	st, err := state.Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	const id = "ctr-control-gone"
	saveCgroupControlTestState(t, st, id, cmd.Process.Pid, start, "/rootfs")

	if err := cmd.Process.Kill(); err != nil {
		t.Fatal(err)
	}
	_ = cmd.Wait()

	called := false
	_, err = controlRunningCgroup(st, id, "test", func(string) error {
		called = true
		return nil
	})
	if called {
		t.Fatal("control callback ran for an already reaped generation")
	}
	if !errors.Is(err, ErrProcessNotFound) {
		t.Fatalf("error=%v, want ErrProcessNotFound", err)
	}
}

func TestControlRunningCgroupPreservesControlError(t *testing.T) {
	cmd, start := startCgroupControlTestProcess(t)
	defer cleanupCgroupControlTestProcess(cmd)

	st, err := state.Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	const id = "ctr-control-error"
	saveCgroupControlTestState(t, st, id, cmd.Process.Pid, start, "/rootfs")

	cause := errors.New("control write failed")
	resolved, err := controlRunningCgroup(st, id, "test", func(string) error { return cause })
	if resolved != nil {
		t.Fatalf("resolved=%+v, want nil after control failure", resolved)
	}
	if !errors.Is(err, cause) {
		t.Fatalf("error=%v, want wrapped control cause", err)
	}
}

func startCgroupControlTestProcess(t *testing.T) (*exec.Cmd, uint64) {
	t.Helper()
	cmd := exec.Command("sleep", "30")
	if err := cmd.Start(); err != nil {
		t.Fatalf("start child: %v", err)
	}
	start, err := ProcessStartTime(cmd.Process.Pid)
	if err != nil {
		_ = cmd.Process.Kill()
		_ = cmd.Wait()
		t.Fatalf("ProcessStartTime: %v", err)
	}
	return cmd, start
}

func cleanupCgroupControlTestProcess(cmd *exec.Cmd) {
	if cmd == nil || cmd.Process == nil {
		return
	}
	if IsRunning(cmd.Process.Pid) {
		_ = cmd.Process.Kill()
	}
	_ = cmd.Wait()
}

func saveCgroupControlTestState(t *testing.T, st *state.Store, id string, pid int, start uint64, rootfs string) {
	t.Helper()
	if err := st.Save(&state.Container{
		ID:           id,
		Status:       state.StatusRunning,
		PID:          pid,
		PIDStartTime: start,
		RootFS:       rootfs,
		CreatedAt:    time.Now(),
	}); err != nil {
		t.Fatalf("save running state: %v", err)
	}
}
