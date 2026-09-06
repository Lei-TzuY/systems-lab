//go:build linux

package container

import (
	"bufio"
	"encoding/json"
	"errors"
	"io"
	"os"
	"os/exec"
	"os/signal"
	"strconv"
	"strings"
	"syscall"
	"testing"
	"time"

	"minicontainer/internal/events"
	"golang.org/x/sys/unix"
)

func readAllExecEvents(t *testing.T) []events.Event {
	t.Helper()
	f, err := os.Open(events.LogPath())
	if err != nil {
		t.Fatal(err)
	}
	defer f.Close()
	dec := json.NewDecoder(f)
	var out []events.Event
	for {
		var evt events.Event
		if err := dec.Decode(&evt); err != nil {
			if errors.Is(err, io.EOF) {
				break
			}
			t.Fatal(err)
		}
		out = append(out, evt)
	}
	return out
}

func TestRunExecPayloadSignalsAfterSuccessfulStartEvenOnNonzeroExit(t *testing.T) {
	readPipe, writePipe, err := os.Pipe()
	if err != nil {
		t.Fatal(err)
	}

	err = runExecPayloadWithStartSignal(
		[]string{"sh", "-c", "exit 23"},
		[]string{"PATH=/bin:/usr/bin"},
		nil,
		os.Stdout,
		os.Stderr,
		writePipe,
	)
	var exitErr *exec.ExitError
	if !errors.As(err, &exitErr) || exitErr.ExitCode() != 23 {
		t.Fatalf("payload exit=%v, want code 23", err)
	}
	if err := awaitExecPayloadStarted(readPipe); err != nil {
		t.Fatalf("nonzero payload did not prove successful Start: %v", err)
	}
}

func TestRunExecPayloadDoesNotSignalWhenStartFails(t *testing.T) {
	readPipe, writePipe, err := os.Pipe()
	if err != nil {
		t.Fatal(err)
	}

	err = runExecPayloadWithStartSignal(
		[]string{"/definitely/not/a/minicontainer-command"},
		nil,
		nil,
		os.Stdout,
		os.Stderr,
		writePipe,
	)
	if err == nil {
		t.Fatal("missing payload unexpectedly started")
	}
	if err := awaitExecPayloadStarted(readPipe); err == nil {
		t.Fatal("failed payload Start produced success proof")
	}
}

func TestExecPayloadSignalForwardingHelper(t *testing.T) {
	if os.Getenv("MINICONTAINER_TEST_EXEC_SIGNAL_FORWARD") != "1" {
		return
	}
	err := runExecPayloadWithStartSignal(
		[]string{"sh", "-c", "parent=$PPID; trap 'exit 0' USR1; echo ready; exec 1>&-; while kill -0 \"$parent\" 2>/dev/null; do :; done; exit 78"},
		[]string{"PATH=/bin:/usr/bin"},
		nil,
		os.Stdout,
		os.Stderr,
		nil,
	)
	if err != nil {
		os.Exit(77)
	}
	os.Exit(0)
}

func TestRunExecPayloadForwardsSignalFromExecInit(t *testing.T) {
	cmd := exec.Command(os.Args[0], "-test.run=^TestExecPayloadSignalForwardingHelper$")
	cmd.Env = append(os.Environ(), "MINICONTAINER_TEST_EXEC_SIGNAL_FORWARD=1")
	stdout, err := cmd.StdoutPipe()
	if err != nil {
		t.Fatal(err)
	}
	cmd.Stderr = os.Stderr
	if err := cmd.Start(); err != nil {
		t.Fatal(err)
	}

	line, err := bufio.NewReader(stdout).ReadString('\n')
	if err != nil {
		_ = cmd.Process.Kill()
		_ = cmd.Wait()
		t.Fatalf("await payload readiness: %v", err)
	}
	if strings.TrimSpace(line) != "ready" {
		_ = cmd.Process.Kill()
		_ = cmd.Wait()
		t.Fatalf("unexpected payload readiness %q", line)
	}
	if err := cmd.Process.Signal(syscall.SIGUSR1); err != nil {
		_ = cmd.Process.Kill()
		_ = cmd.Wait()
		t.Fatalf("signal exec-init helper: %v", err)
	}
	if err := cmd.Wait(); err != nil {
		t.Fatalf("exec-init helper did not forward SIGUSR1 to payload: %v", err)
	}
}

func TestExecPayloadProcessGroupForwardingDescendant(t *testing.T) {
	if os.Getenv("MINICONTAINER_TEST_EXEC_GROUP_DESCENDANT") != "1" {
		return
	}

	signals := make(chan os.Signal, 1)
	signal.Notify(signals, syscall.SIGUSR1)
	defer signal.Stop(signals)

	_, _ = os.Stdout.WriteString("descendant-ready\n")
	select {
	case <-signals:
		_, _ = os.Stdout.WriteString("descendant-signaled\n")
		os.Exit(0)
	case <-time.After(5 * time.Second):
		os.Exit(82)
	}
}

func TestExecPayloadProcessGroupForwardingLeader(t *testing.T) {
	if os.Getenv("MINICONTAINER_TEST_EXEC_GROUP_LEADER") != "1" {
		return
	}

	// The workload leader deliberately handles SIGUSR1 without exiting. A
	// direct-PID forward therefore cannot complete the descendant handshake.
	leaderSignals := make(chan os.Signal, 1)
	signal.Notify(leaderSignals, syscall.SIGUSR1)
	defer signal.Stop(leaderSignals)

	cmd := exec.Command(os.Args[0], "-test.run=^TestExecPayloadProcessGroupForwardingDescendant$")
	cmd.Env = append(os.Environ(), "MINICONTAINER_TEST_EXEC_GROUP_DESCENDANT=1")
	cmd.Stdout = os.Stdout
	cmd.Stderr = os.Stderr
	if err := cmd.Run(); err != nil {
		os.Exit(81)
	}
	os.Exit(0)
}

func TestExecPayloadProcessGroupForwardingHelper(t *testing.T) {
	if os.Getenv("MINICONTAINER_TEST_EXEC_GROUP_FORWARD") != "1" {
		return
	}

	err := runExecPayloadWithStartSignal(
		[]string{os.Args[0], "-test.run=^TestExecPayloadProcessGroupForwardingLeader$"},
		append(os.Environ(), "MINICONTAINER_TEST_EXEC_GROUP_LEADER=1"),
		nil,
		os.Stdout,
		os.Stderr,
		nil,
	)
	if err != nil {
		os.Exit(77)
	}
	os.Exit(0)
}

func TestRunExecPayloadForwardsSignalToDescendantProcessGroup(t *testing.T) {
	cmd := exec.Command(os.Args[0], "-test.run=^TestExecPayloadProcessGroupForwardingHelper$")
	cmd.Env = append(os.Environ(), "MINICONTAINER_TEST_EXEC_GROUP_FORWARD=1")
	stdout, err := cmd.StdoutPipe()
	if err != nil {
		t.Fatal(err)
	}
	cmd.Stderr = os.Stderr
	if err := cmd.Start(); err != nil {
		t.Fatal(err)
	}

	reader := bufio.NewReader(stdout)
	line, err := reader.ReadString('\n')
	if err != nil {
		_ = cmd.Process.Kill()
		_ = cmd.Wait()
		t.Fatalf("await descendant readiness: %v", err)
	}
	if strings.TrimSpace(line) != "descendant-ready" {
		_ = cmd.Process.Kill()
		_ = cmd.Wait()
		t.Fatalf("unexpected descendant readiness %q", line)
	}
	if err := cmd.Process.Signal(syscall.SIGUSR1); err != nil {
		_ = cmd.Process.Kill()
		_ = cmd.Wait()
		t.Fatalf("signal exec-init helper: %v", err)
	}
	line, err = reader.ReadString('\n')
	if err != nil {
		_ = cmd.Process.Kill()
		_ = cmd.Wait()
		t.Fatalf("await descendant signal acknowledgement: %v", err)
	}
	if strings.TrimSpace(line) != "descendant-signaled" {
		_ = cmd.Process.Kill()
		_ = cmd.Wait()
		t.Fatalf("unexpected descendant acknowledgement %q", line)
	}
	if err := cmd.Wait(); err != nil {
		t.Fatalf("exec-init helper did not complete after descendant group signal: %v", err)
	}
}

func TestExecPayloadCleanupDescendant(t *testing.T) {
	if os.Getenv("MINICONTAINER_TEST_EXEC_CLEANUP_DESCENDANT") != "1" {
		return
	}
	_, _ = os.Stdout.WriteString("cleanup-descendant-ready " + strconv.Itoa(os.Getpid()) + "\n")
	select {
	case <-time.After(10 * time.Second):
		os.Exit(86)
	}
}

func TestExecPayloadCleanupLeader(t *testing.T) {
	if os.Getenv("MINICONTAINER_TEST_EXEC_CLEANUP_LEADER") != "1" {
		return
	}

	cmd := exec.Command(os.Args[0], "-test.run=^TestExecPayloadCleanupDescendant$")
	cmd.Env = append(os.Environ(), "MINICONTAINER_TEST_EXEC_CLEANUP_DESCENDANT=1")
	stdout, err := cmd.StdoutPipe()
	if err != nil {
		os.Exit(83)
	}
	cmd.Stderr = os.Stderr
	if err := cmd.Start(); err != nil {
		os.Exit(84)
	}
	line, err := bufio.NewReader(stdout).ReadString('\n')
	if err != nil {
		os.Exit(85)
	}
	_, _ = os.Stdout.WriteString(line)
	os.Exit(0)
}

func TestExecPayloadCleanupHelper(t *testing.T) {
	if os.Getenv("MINICONTAINER_TEST_EXEC_CLEANUP_HELPER") != "1" {
		return
	}

	err := runExecPayloadWithStartSignal(
		[]string{os.Args[0], "-test.run=^TestExecPayloadCleanupLeader$"},
		append(os.Environ(), "MINICONTAINER_TEST_EXEC_CLEANUP_LEADER=1"),
		nil,
		os.Stdout,
		os.Stderr,
		nil,
	)
	if err != nil {
		os.Exit(87)
	}
	os.Exit(0)
}

func TestRunExecPayloadCleansDescendantAfterLeaderExit(t *testing.T) {
	cmd := exec.Command(os.Args[0], "-test.run=^TestExecPayloadCleanupHelper$")
	cmd.Env = append(os.Environ(), "MINICONTAINER_TEST_EXEC_CLEANUP_HELPER=1")
	stdout, err := cmd.StdoutPipe()
	if err != nil {
		t.Fatal(err)
	}
	cmd.Stderr = os.Stderr
	if err := cmd.Start(); err != nil {
		t.Fatal(err)
	}

	reader := bufio.NewReader(stdout)
	line, err := reader.ReadString('\n')
	if err != nil {
		_ = cmd.Process.Kill()
		_ = cmd.Wait()
		t.Fatalf("await cleanup descendant readiness: %v", err)
	}
	fields := strings.Fields(strings.TrimSpace(line))
	if len(fields) != 2 || fields[0] != "cleanup-descendant-ready" {
		_ = cmd.Process.Kill()
		_ = cmd.Wait()
		t.Fatalf("unexpected cleanup readiness %q", line)
	}
	descendantPID, err := strconv.Atoi(fields[1])
	if err != nil {
		_ = cmd.Process.Kill()
		_ = cmd.Wait()
		t.Fatalf("parse cleanup descendant pid %q: %v", fields[1], err)
	}

	pidfd, err := unix.PidfdOpen(descendantPID, 0)
	if err != nil && !errors.Is(err, syscall.ESRCH) {
		_ = cmd.Process.Kill()
		_ = cmd.Wait()
		t.Fatalf("open cleanup descendant pidfd: %v", err)
	}
	if pidfd >= 0 {
		defer unix.Close(pidfd)
	}

	if err := cmd.Wait(); err != nil {
		t.Fatalf("exec-init helper did not complete after leader exit: %v", err)
	}
	if pidfd < 0 {
		return
	}

	fds := []unix.PollFd{{Fd: int32(pidfd), Events: unix.POLLIN}}
	n, err := unix.Poll(fds, 5000)
	if err != nil {
		t.Fatalf("wait for cleanup descendant exit: %v", err)
	}
	if n != 1 || fds[0].Revents&unix.POLLIN == 0 {
		t.Fatalf("cleanup descendant %d outlived exec completion", descendantPID)
	}
}

func TestRunExecInitCommandRecordsStartAndNonzeroCompletion(t *testing.T) {
	t.Setenv("HOME", t.TempDir())
	const containerID = "exec-proof-nonzero"
	if err := events.Publish(events.EventExec, containerID, "rootfs", "exec [sh]"); err != nil {
		t.Fatalf("stage exec: %v", err)
	}

	// runExecInitCommand has no pre-existing ExtraFiles here, so its proof pipe
	// is fd 3. The helper writes the exact proof byte, then exits nonzero.
	cmd := exec.Command("sh", "-c", "printf '\\347' >&3; exit 29")
	err := runExecInitCommand(cmd)
	var exitErr *exec.ExitError
	if !errors.As(err, &exitErr) || exitErr.ExitCode() != 29 {
		t.Fatalf("exec-init exit=%v, want code 29", err)
	}

	got := readAllExecEvents(t)
	if len(got) != 2 {
		t.Fatalf("events=%+v, want start and terminal outcome", got)
	}
	if got[0].Type != events.EventExec || got[0].ContainerID != containerID {
		t.Fatalf("start event=%+v", got[0])
	}
	if got[1].Type != events.EventExecExit || got[1].ContainerID != containerID || !strings.Contains(got[1].Message, "exit_code=29") {
		t.Fatalf("terminal event=%+v", got[1])
	}
}

func TestRunExecInitCommandRecordsFailureWhenProofMissing(t *testing.T) {
	t.Setenv("HOME", t.TempDir())
	const containerID = "exec-proof-missing"
	if err := events.Publish(events.EventExec, containerID, "rootfs", "exec [failed]"); err != nil {
		t.Fatal(err)
	}

	cmd := exec.Command("sh", "-c", "exit 31")
	err := runExecInitCommand(cmd)
	var exitErr *exec.ExitError
	if !errors.As(err, &exitErr) || exitErr.ExitCode() != 31 {
		t.Fatalf("exec-init exit=%v, want code 31", err)
	}
	got := readAllExecEvents(t)
	if len(got) != 1 || got[0].Type != events.EventExecFailed || got[0].ContainerID != containerID {
		t.Fatalf("events=%+v, want one exec_failed and no started event", got)
	}
	if !strings.Contains(got[0].Message, "payload start") {
		t.Fatalf("failure cause missing: %+v", got[0])
	}
}

func TestPayloadEnvironmentStripsExecStartedFD(t *testing.T) {
	got := strings.Join(payloadEnvironment([]string{
		"PATH=/bin:/usr/bin",
		execStartedFDKey + "=9",
		"VISIBLE=yes",
	}), "\n")
	if strings.Contains(got, execStartedFDKey+"=") {
		t.Fatalf("exec payload-start descriptor leaked into payload env: %q", got)
	}
	if !strings.Contains(got, "VISIBLE=yes") {
		t.Fatalf("ordinary environment entry lost: %q", got)
	}
}
