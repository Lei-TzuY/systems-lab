//go:build linux

// internal/container/exec_linux.go
//
// `minictl exec` — Enter an Existing Container
// ─────────────────────────────────────────────
// setns(CLONE_NEWPID) changes only the PID namespace used for subsequently
// created children; it does not move the calling process itself. Therefore the
// exec-init helper must join namespaces first and then spawn the payload as a
// child. Directly calling execve after PID setns would leave the payload in the
// caller's original PID namespace.

package container

import (
	"fmt"
	"io"
	"os"
	"os/exec"
	"path/filepath"
	"runtime"
	"strconv"
	"strings"

	"minicontainer/internal/events"
	"minicontainer/internal/state"
	"golang.org/x/sys/unix"
)

type ExecConfig struct {
	ContainerPID int
	RootFS       string
	Command      []string
	Debug        bool
}

const (
	execSentinelKey  = "MINICONTAINER_EXEC"
	execSentinelEnv  = execSentinelKey + "=1"
	execStartTimeKey = "MINICONTAINER_EXEC_START_TIME"
)

type execNamespaceTarget struct {
	name string
	flag int
	fd   int
}

type execTargets struct {
	rootFD    int
	startTime uint64
	process   *ProcessHandle
	ns        []execNamespaceTarget
}

func (t *execTargets) close() {
	if t == nil {
		return
	}
	if t.rootFD >= 0 {
		_ = unix.Close(t.rootFD)
		t.rootFD = -1
	}
	for i := range t.ns {
		if t.ns[i].fd >= 0 {
			_ = unix.Close(t.ns[i].fd)
			t.ns[i].fd = -1
		}
	}
	if t.process != nil {
		_ = t.process.Close()
		t.process = nil
	}
}

func Exec(cfg ExecConfig) error {
	if cfg.ContainerPID <= 0 {
		return fmt.Errorf("invalid container PID %d", cfg.ContainerPID)
	}
	if len(cfg.Command) == 0 || cfg.Command[0] == "" {
		return fmt.Errorf("exec command is empty")
	}
	expectedStartTime, err := persistedExecStartTime(cfg.ContainerPID, cfg.RootFS)
	if err != nil {
		return err
	}
	// Bind observability to the same verified process generation that the exec
	// bootstrap will enter. Direct/internal Exec callers without a staged CLI
	// event remain unaffected because the binder is a no-op when nothing is staged.
	if err := events.BindPendingExecAttribution(cfg.ContainerPID, expectedStartTime, cfg.Command); err != nil {
		return fmt.Errorf("bind exec lifecycle attribution: %w", err)
	}
	if cfg.Debug {
		fmt.Printf("[exec] entering namespaces of PID %d\n", cfg.ContainerPID)
	}
	self, err := os.Executable()
	if err != nil {
		return fmt.Errorf("resolve executable: %w", err)
	}
	childArgs := append([]string{"exec", strconv.Itoa(cfg.ContainerPID), cfg.RootFS}, cfg.Command...)
	cmd := exec.Command(self, childArgs...)
	cmd.Stdin = os.Stdin
	cmd.Stdout = os.Stdout
	cmd.Stderr = os.Stderr
	cmd.Env = append(os.Environ(), execSentinelEnv, fmt.Sprintf("%s=%d", execStartTimeKey, expectedStartTime))
	if err := runExecInitCommand(cmd); err != nil {
		if exitErr, ok := err.(*exec.ExitError); ok {
			os.Exit(exitErr.ExitCode())
		}
		return err
	}
	return nil
}

func persistedExecStartTime(containerPID int, rootFS string) (uint64, error) {
	store, err := state.Open(state.DefaultDir())
	if err != nil {
		return 0, fmt.Errorf("open container state for exec identity: %w", err)
	}
	records, err := store.List()
	if err != nil {
		return 0, fmt.Errorf("list container state for exec identity: %w", err)
	}
	var match *state.Container
	for _, rec := range records {
		if rec.PID != containerPID || rec.RootFS != rootFS || rec.Status != state.StatusRunning {
			continue
		}
		if match != nil {
			return 0, fmt.Errorf("ambiguous persisted exec identity for PID %d", containerPID)
		}
		match = rec
	}
	if match == nil {
		return 0, fmt.Errorf("no running container state matches exec PID %d and rootfs %q", containerPID, rootFS)
	}
	if match.PIDStartTime == 0 {
		return 0, fmt.Errorf("container %s has no persisted PID start time; refusing unsafe exec", match.ID)
	}
	actual, err := ProcessStartTime(containerPID)
	if err != nil {
		return 0, fmt.Errorf("verify persisted exec target PID %d: %w", containerPID, err)
	}
	if actual != match.PIDStartTime {
		return 0, fmt.Errorf("container %s PID %d was reused: persisted start time %d, current %d", match.ID, containerPID, match.PIDStartTime, actual)
	}
	return match.PIDStartTime, nil
}

func requireExecTargetAlive(targets *execTargets, phase string) error {
	if targets == nil || targets.process == nil {
		return fmt.Errorf("exec target process handle is unavailable %s", phase)
	}
	exited, err := targets.process.WaitExit(0)
	if err != nil {
		return fmt.Errorf("prove exec target PID %d alive %s: %w", targets.process.PID, phase, err)
	}
	if exited {
		return fmt.Errorf("exec target PID %d exited %s", targets.process.PID, phase)
	}
	return nil
}

func openExecTargets(containerPID int, expectedStartTime uint64) (*execTargets, error) {
	if containerPID <= 0 {
		return nil, fmt.Errorf("invalid container PID %d", containerPID)
	}
	if expectedStartTime == 0 {
		return nil, fmt.Errorf("missing persisted PID start time for exec target %d", containerPID)
	}

	process, err := OpenProcessHandle(containerPID, expectedStartTime)
	if err != nil {
		return nil, fmt.Errorf("exec target PID %d does not match persisted identity %d: %w", containerPID, expectedStartTime, err)
	}
	targets := &execTargets{rootFD: -1, startTime: expectedStartTime, process: process}
	fail := func(err error) (*execTargets, error) {
		targets.close()
		return nil, err
	}
	if err := requireExecTargetAlive(targets, "before namespace capture"); err != nil {
		return fail(err)
	}

	rootPath := fmt.Sprintf("/proc/%d/root", containerPID)
	rootFD, err := unix.Open(rootPath, unix.O_PATH|unix.O_DIRECTORY|unix.O_CLOEXEC, 0)
	if err != nil {
		return fail(fmt.Errorf("open %s: %w", rootPath, err))
	}
	targets.rootFD = rootFD

	nsSpecs := []struct {
		name string
		flag int
	}{
		{"net", unix.CLONE_NEWNET},
		{"ipc", unix.CLONE_NEWIPC},
		{"uts", unix.CLONE_NEWUTS},
		{"pid", unix.CLONE_NEWPID},
		{"mnt", unix.CLONE_NEWNS},
	}
	for _, spec := range nsSpecs {
		nsPath := fmt.Sprintf("/proc/%d/ns/%s", containerPID, spec.name)
		fd, err := unix.Open(nsPath, unix.O_RDONLY|unix.O_CLOEXEC, 0)
		if err != nil {
			if err == unix.EACCES || err == unix.EPERM {
				return fail(fmt.Errorf("cannot open %s: permission denied; run exec with sufficient namespace privileges", nsPath))
			}
			return fail(fmt.Errorf("open %s: %w (is the container still running?)", nsPath, err))
		}
		targets.ns = append(targets.ns, execNamespaceTarget{name: spec.name, flag: spec.flag, fd: fd})
	}

	// The pidfd proves liveness of the exact process generation while the final
	// start-time check proves that every pathname-based /proc capture above still
	// referred to the same numeric PID. Both are required: a pidfd alone cannot
	// bind later /proc/<pid> pathname opens to the original process after reuse.
	endStartTime, err := ProcessStartTime(containerPID)
	if err != nil {
		return fail(fmt.Errorf("recheck exec target identity for PID %d: %w", containerPID, err))
	}
	if endStartTime != expectedStartTime {
		return fail(fmt.Errorf("exec target PID %d changed identity during namespace capture", containerPID))
	}
	if err := requireExecTargetAlive(targets, "after namespace capture"); err != nil {
		return fail(err)
	}
	return targets, nil
}

func ExecInit(containerPID int, _ string, command []string, debug bool) error {
	if len(command) == 0 || command[0] == "" {
		return fmt.Errorf("exec command is empty")
	}
	expectedRaw := os.Getenv(execStartTimeKey)
	expectedStartTime, err := strconv.ParseUint(expectedRaw, 10, 64)
	if err != nil || expectedStartTime == 0 {
		return fmt.Errorf("invalid internal exec target identity %q", expectedRaw)
	}
	startWriter, err := execPayloadStartWriterFromEnv()
	if err != nil {
		return err
	}
	defer startWriter.Close()

	runtime.LockOSThread()
	if err := prepareExecThread(); err != nil {
		return err
	}
	if debug {
		fmt.Printf("[exec-init] attaching to namespaces of host PID %d\n", containerPID)
	}
	targets, err := openExecTargets(containerPID, expectedStartTime)
	if err != nil {
		return err
	}
	defer targets.close()

	for _, ns := range targets.ns {
		if err := unix.Setns(ns.fd, ns.flag); err != nil {
			return fmt.Errorf("setns(%s): %w", ns.name, err)
		}
		if debug {
			fmt.Printf("[exec-init] joined %s namespace\n", ns.name)
		}
	}
	if err := unix.Fchdir(targets.rootFD); err != nil {
		return fmt.Errorf("fchdir container root: %w", err)
	}
	if err := unix.Chroot("."); err != nil {
		return fmt.Errorf("chroot container root: %w", err)
	}
	if err := unix.Chdir("/"); err != nil {
		return fmt.Errorf("chdir /: %w", err)
	}

	// setns and chroot can take long enough for the target init to exit. A
	// stopped generation must not admit a new exec payload merely because its
	// namespace/root descriptors were captured while it was still alive.
	if err := requireExecTargetAlive(targets, "immediately before payload spawn"); err != nil {
		return err
	}
	if debug {
		fmt.Printf("[exec-init] spawning payload in target PID namespace: %v\n", command)
	}
	err = runExecPayloadWithStartSignal(command, payloadEnvironment(os.Environ()), os.Stdin, os.Stdout, os.Stderr, startWriter)
	if err == nil {
		return nil
	}
	if exitErr, ok := err.(*exec.ExitError); ok {
		os.Exit(exitErr.ExitCode())
	}
	return fmt.Errorf("start exec payload: %w", err)
}

func runExecPayload(command, env []string, stdin io.Reader, stdout, stderr io.Writer) error {
	return runExecPayloadWithStartSignal(command, env, stdin, stdout, stderr, nil)
}

func payloadEnvironment(env []string) []string {
	out := make([]string, 0, len(env))
	for _, entry := range env {
		if strings.HasPrefix(entry, execSentinelKey+"=") ||
			strings.HasPrefix(entry, execStartTimeKey+"=") ||
			strings.HasPrefix(entry, execStartedFDKey+"=") ||
			strings.HasPrefix(entry, "MINICONTAINER_INIT=") {
			continue
		}
		out = append(out, entry)
	}
	return out
}

func IsRunning(pid int) bool {
	data, err := os.ReadFile(fmt.Sprintf("/proc/%d/status", pid))
	if err != nil {
		return false
	}
	for _, line := range strings.Split(string(data), "\n") {
		if strings.HasPrefix(line, "State:") {
			return !strings.Contains(line, "Z") && !strings.Contains(line, "X")
		}
	}
	return true
}

func ContainerCwd(pid int) string {
	cwd, err := filepath.EvalSymlinks(fmt.Sprintf("/proc/%d/cwd", pid))
	if err != nil {
		return "/"
	}
	return cwd
}
