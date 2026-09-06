//go:build linux

package main

import (
	"errors"
	"fmt"
	"os"
	"os/exec"
	"os/signal"
	"path/filepath"
	"runtime"
	"strconv"
	"strings"
	"syscall"
	"testing"
	"time"
)

const (
	initSupervisorTestRole = "MINICONTAINER_TEST_INIT_SUPERVISOR_ROLE"
	initSupervisorTestDir  = "MINICONTAINER_TEST_INIT_SUPERVISOR_DIR"
)

func TestWrapContainerInitPayload(t *testing.T) {
	oldArgs := os.Args
	oldInit := os.Getenv("MINICONTAINER_INIT")
	t.Cleanup(func() {
		os.Args = oldArgs
		_ = os.Setenv("MINICONTAINER_INIT", oldInit)
	})

	os.Args = []string{"/host/minictl", "run", "--hostname", "demo", "/rootfs", "/bin/app", "arg1"}
	if err := os.Setenv("MINICONTAINER_INIT", "1"); err != nil {
		t.Fatal(err)
	}
	wrapContainerInitPayload()

	want := []string{"/host/minictl", "run", "--hostname", "demo", "/rootfs", "/proc/self/exe", initSupervisorArg, "/bin/app", "arg1"}
	if len(os.Args) != len(want) {
		t.Fatalf("wrapped args len=%d want=%d: %q", len(os.Args), len(want), os.Args)
	}
	for i := range want {
		if os.Args[i] != want[i] {
			t.Fatalf("wrapped arg[%d]=%q want=%q; all=%q", i, os.Args[i], want[i], os.Args)
		}
	}
}

func TestContainerInitSupervisorReapsOrphan(t *testing.T) {
	role := os.Getenv(initSupervisorTestRole)
	if role != "" {
		runInitSupervisorReapHelper(t, role)
		return
	}

	dir := t.TempDir()
	cmd := exec.Command(os.Args[0], "-test.run=^TestContainerInitSupervisorReapsOrphan$")
	cmd.Env = helperEnv(initSupervisorTestRole, "supervisor", initSupervisorTestDir, dir)
	if err := cmd.Start(); err != nil {
		t.Fatalf("start supervisor helper: %v", err)
	}
	finished := false
	defer func() {
		if !finished && cmd.Process != nil {
			_ = cmd.Process.Kill()
			_ = cmd.Wait()
		}
	}()

	pidData := waitForFile(t, filepath.Join(dir, "orphan.pid"))
	orphanPID, err := strconv.Atoi(string(pidData))
	if err != nil || orphanPID <= 0 {
		t.Fatalf("invalid orphan pid %q: %v", pidData, err)
	}
	if err := os.WriteFile(filepath.Join(dir, "release-orphan"), []byte("1"), 0o600); err != nil {
		t.Fatal(err)
	}
	waitForProcessGone(t, orphanPID)

	if err := os.WriteFile(filepath.Join(dir, "finish-payload"), []byte("1"), 0o600); err != nil {
		t.Fatal(err)
	}
	if err := cmd.Wait(); err != nil {
		t.Fatalf("supervisor helper failed: %v", err)
	}
	finished = true
}

func runInitSupervisorReapHelper(t *testing.T, role string) {
	t.Helper()
	dir := os.Getenv(initSupervisorTestDir)
	switch role {
	case "supervisor":
		if err := os.Setenv(initSupervisorTestRole, "payload"); err != nil {
			os.Exit(120)
		}
		code, err := runContainerInitSupervisor([]string{os.Args[0], "-test.run=^TestContainerInitSupervisorReapsOrphan$"})
		if err != nil {
			fmt.Fprintln(os.Stderr, err)
			os.Exit(121)
		}
		os.Exit(code)
	case "payload":
		cmd := exec.Command(os.Args[0], "-test.run=^TestContainerInitSupervisorReapsOrphan$")
		cmd.Env = helperEnv(initSupervisorTestRole, "orphan-parent", initSupervisorTestDir, dir)
		if err := cmd.Run(); err != nil {
			os.Exit(122)
		}
		waitForPathInHelper(filepath.Join(dir, "finish-payload"))
		return
	case "orphan-parent":
		cmd := exec.Command(os.Args[0], "-test.run=^TestContainerInitSupervisorReapsOrphan$")
		cmd.Env = helperEnv(initSupervisorTestRole, "orphan", initSupervisorTestDir, dir)
		if err := cmd.Start(); err != nil {
			os.Exit(123)
		}
		waitForPathInHelper(filepath.Join(dir, "orphan-ready"))
		os.Exit(0)
	case "orphan":
		initialParent := os.Getppid()
		if err := os.WriteFile(filepath.Join(dir, "orphan-ready"), []byte(strconv.Itoa(initialParent)), 0o600); err != nil {
			os.Exit(124)
		}
		deadline := time.Now().Add(5 * time.Second)
		for os.Getppid() == initialParent {
			if time.Now().After(deadline) {
				os.Exit(125)
			}
			runtime.Gosched()
		}
		if err := os.WriteFile(filepath.Join(dir, "orphan.pid"), []byte(strconv.Itoa(os.Getpid())), 0o600); err != nil {
			os.Exit(126)
		}
		waitForPathInHelper(filepath.Join(dir, "release-orphan"))
		return
	default:
		os.Exit(127)
	}
}

func TestContainerInitSupervisorForwardsSignal(t *testing.T) {
	role := os.Getenv(initSupervisorTestRole)
	if role != "" {
		runInitSupervisorSignalHelper(t, role)
		return
	}

	dir := t.TempDir()
	cmd := exec.Command(os.Args[0], "-test.run=^TestContainerInitSupervisorForwardsSignal$")
	cmd.Env = helperEnv(initSupervisorTestRole, "signal-supervisor", initSupervisorTestDir, dir)
	if err := cmd.Start(); err != nil {
		t.Fatalf("start signal supervisor helper: %v", err)
	}
	finished := false
	defer func() {
		if !finished && cmd.Process != nil {
			_ = cmd.Process.Kill()
			_ = cmd.Wait()
		}
	}()

	waitForFile(t, filepath.Join(dir, "payload-ready"))
	if err := cmd.Process.Signal(syscall.SIGUSR1); err != nil {
		t.Fatalf("signal supervisor: %v", err)
	}
	waitForFile(t, filepath.Join(dir, "payload-signaled"))
	if err := cmd.Wait(); err != nil {
		t.Fatalf("signal supervisor helper failed: %v", err)
	}
	finished = true
}

func runInitSupervisorSignalHelper(t *testing.T, role string) {
	t.Helper()
	dir := os.Getenv(initSupervisorTestDir)
	switch role {
	case "signal-supervisor":
		if err := os.Setenv(initSupervisorTestRole, "signal-payload"); err != nil {
			os.Exit(130)
		}
		code, err := runContainerInitSupervisor([]string{os.Args[0], "-test.run=^TestContainerInitSupervisorForwardsSignal$"})
		if err != nil {
			fmt.Fprintln(os.Stderr, err)
			os.Exit(131)
		}
		os.Exit(code)
	case "signal-payload":
		sigCh := make(chan os.Signal, 1)
		signal.Notify(sigCh, syscall.SIGUSR1)
		defer signal.Stop(sigCh)
		if err := os.WriteFile(filepath.Join(dir, "payload-ready"), []byte("1"), 0o600); err != nil {
			os.Exit(132)
		}
		<-sigCh
		if err := os.WriteFile(filepath.Join(dir, "payload-signaled"), []byte("1"), 0o600); err != nil {
			os.Exit(133)
		}
		return
	default:
		os.Exit(134)
	}
}

func helperEnv(k1, v1, k2, v2 string) []string {
	prefix1 := k1 + "="
	prefix2 := k2 + "="
	env := make([]string, 0, len(os.Environ())+2)
	for _, entry := range os.Environ() {
		if strings.HasPrefix(entry, prefix1) || strings.HasPrefix(entry, prefix2) {
			continue
		}
		env = append(env, entry)
	}
	env = append(env, k1+"="+v1, k2+"="+v2)
	return env
}

func waitForFile(t *testing.T, path string) []byte {
	t.Helper()
	deadline := time.Now().Add(5 * time.Second)
	for {
		data, err := os.ReadFile(path)
		if err == nil && len(data) > 0 {
			return data
		}
		if err != nil && !os.IsNotExist(err) {
			t.Fatalf("read %s: %v", path, err)
		}
		if time.Now().After(deadline) {
			t.Fatalf("timed out waiting for non-empty %s", path)
		}
		runtime.Gosched()
	}
}

func waitForPathInHelper(path string) {
	deadline := time.Now().Add(5 * time.Second)
	for {
		if _, err := os.Stat(path); err == nil {
			return
		} else if !os.IsNotExist(err) {
			os.Exit(140)
		}
		if time.Now().After(deadline) {
			os.Exit(141)
		}
		runtime.Gosched()
	}
}

func waitForProcessGone(t *testing.T, pid int) {
	t.Helper()
	path := filepath.Join("/proc", strconv.Itoa(pid), "stat")
	deadline := time.Now().Add(5 * time.Second)
	for {
		_, err := os.Stat(path)
		if os.IsNotExist(err) || errors.Is(err, syscall.ESRCH) {
			return
		}
		if err != nil {
			t.Fatalf("stat orphan process %d: %v", pid, err)
		}
		if time.Now().After(deadline) {
			t.Fatalf("orphan process %d remained in /proc; supervisor did not reap it", pid)
		}
		runtime.Gosched()
	}
}
