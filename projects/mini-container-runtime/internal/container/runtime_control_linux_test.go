//go:build linux

package container

import (
	"errors"
	"os"
	"os/exec"
	"testing"
	"time"

	"minicontainer/internal/cgroups"
	"minicontainer/internal/state"
)

func TestResourceLimitsRequested(t *testing.T) {
	tests := []struct {
		name string
		cfg  Config
		want bool
	}{
		{name: "none", cfg: Config{}, want: false},
		{name: "memory", cfg: Config{Memory: 1}, want: true},
		{name: "cpu weight", cfg: Config{CPUWeight: 1}, want: true},
		{name: "cpu quota", cfg: Config{CPUs: 0.5}, want: true},
		{name: "pids", cfg: Config{PidsLimit: 1}, want: true},
		{name: "invalid negative memory still explicit", cfg: Config{Memory: -1}, want: true},
		{name: "invalid negative cpu quota still explicit", cfg: Config{CPUs: -1}, want: true},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if got := resourceLimitsRequested(tt.cfg); got != tt.want {
				t.Fatalf("resourceLimitsRequested(%+v)=%v, want %v", tt.cfg, got, tt.want)
			}
		})
	}
}

func TestRuntimeCgroupNameUsesFullManagedIdentity(t *testing.T) {
	managed, err := runtimeCgroupName("ctr", 42, 99, true)
	if err != nil {
		t.Fatal(err)
	}
	wantManaged, err := cgroups.NameForContainerProcess("ctr", 42, 99)
	if err != nil {
		t.Fatal(err)
	}
	if managed != wantManaged {
		t.Fatalf("managed cgroup=%q, want %q", managed, wantManaged)
	}

	unmanaged, err := runtimeCgroupName("", 42, 0, false)
	if err != nil {
		t.Fatal(err)
	}
	if unmanaged != "minicontainer-42" {
		t.Fatalf("unmanaged cgroup=%q, want minicontainer-42", unmanaged)
	}
	if _, err := runtimeCgroupName("", 0, 0, false); err == nil {
		t.Fatal("invalid unmanaged PID accepted")
	}
}

func TestRuntimeControlErrorsSurviveJoin(t *testing.T) {
	setupErr := &runtimeSetupError{err: errors.New("setup failed")}
	stateErr := &runtimeStateError{err: errors.New("state failed")}

	for _, err := range []error{
		setupErr,
		stateErr,
		errors.Join(errors.New("other"), setupErr),
		errors.Join(errors.New("other"), stateErr),
	} {
		if !isRuntimeControlError(err) {
			t.Fatalf("runtime control error not detected through wrapping/join: %v", err)
		}
	}
	if isRuntimeControlError(errors.New("payload failed")) {
		t.Fatal("ordinary payload error misclassified as runtime control failure")
	}
}

func TestAbortRuntimeSetupFailureKillsChildAndStopsLifecycle(t *testing.T) {
	dir := t.TempDir()
	st, err := state.Open(dir)
	if err != nil {
		t.Fatal(err)
	}

	const id = "ctr-required-cgroup"
	created := &state.Container{
		ID:        id,
		Status:    state.StatusCreated,
		RootFS:    "/tmp/rootfs",
		Command:   []string{"sleep", "30"},
		CreatedAt: time.Now(),
	}
	if err := st.Save(created); err != nil {
		t.Fatal(err)
	}

	cmd := exec.Command("sh", "-c", "exec sleep 30")
	if err := cmd.Start(); err != nil {
		t.Fatal(err)
	}
	pid := cmd.Process.Pid
	startTime, err := ProcessStartTime(pid)
	if err != nil {
		_ = cmd.Process.Kill()
		_ = cmd.Wait()
		t.Fatal(err)
	}
	if err := st.MarkRunning(id, pid, startTime, time.Now()); err != nil {
		_ = cmd.Process.Kill()
		_ = cmd.Wait()
		t.Fatal(err)
	}

	readPipe, writePipe, err := os.Pipe()
	if err != nil {
		_ = cmd.Process.Kill()
		_ = cmd.Wait()
		t.Fatal(err)
	}
	defer readPipe.Close()

	cause := errors.New("required cgroup limit rejected")
	err = abortRuntimeSetupFailure(cmd, writePipe, st, id, pid, startTime, cause)
	if !isRuntimeControlError(err) {
		t.Fatalf("setup failure not classified as runtime control error: %v", err)
	}
	var setupErr *runtimeSetupError
	if !errors.As(err, &setupErr) {
		t.Fatalf("runtimeSetupError not discoverable: %v", err)
	}
	if !errors.Is(err, cause) {
		t.Fatalf("original cgroup cause not preserved: %v", err)
	}
	if IsRunning(pid) {
		t.Fatalf("child PID %d still alive after fail-closed abort", pid)
	}

	current, err := st.Get(id)
	if err != nil {
		t.Fatal(err)
	}
	if current.Status != state.StatusStopped {
		t.Fatalf("status=%s, want stopped", current.Status)
	}
	if current.PID != 0 || current.PIDStartTime != 0 {
		t.Fatalf("stopped state retained process identity: pid=%d start=%d", current.PID, current.PIDStartTime)
	}
	if current.ExitCode != -1 {
		t.Fatalf("exit code=%d, want -1 for runtime-aborted child", current.ExitCode)
	}
	if current.FinishedAt == nil {
		t.Fatal("runtime-aborted state missing FinishedAt")
	}
}
