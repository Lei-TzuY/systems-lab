//go:build linux

package main

import (
	"errors"
	"fmt"
	"os"
	"os/exec"
	"os/signal"
	"runtime"
	"syscall"

	"golang.org/x/sys/unix"
)

const initSupervisorArg = "__minicontainer-init-supervisor"

var initSupervisorForwardSignals = []os.Signal{
	syscall.SIGHUP,
	syscall.SIGINT,
	syscall.SIGQUIT,
	syscall.SIGUSR1,
	syscall.SIGUSR2,
	syscall.SIGTERM,
	syscall.SIGTSTP,
	syscall.SIGTTIN,
	syscall.SIGTTOU,
	syscall.SIGCONT,
	syscall.SIGWINCH,
}

// init installs a tiny internal wrapper around the payload command only in the
// re-executed container-init process. ContainerInit still performs all existing
// namespace/rootfs/security setup, then execs /proc/self/exe. That process stays
// PID 1 and supervises the real payload as its child.
func init() {
	if os.Getenv("MINICONTAINER_INIT") == "1" {
		wrapContainerInitPayload()
		return
	}
	if len(os.Args) >= 2 && os.Args[1] == initSupervisorArg {
		code, err := runContainerInitSupervisor(os.Args[2:])
		if err != nil {
			fmt.Fprintf(os.Stderr, "container init supervisor: %v\n", err)
			os.Exit(125)
		}
		os.Exit(code)
	}
}

func wrapContainerInitPayload() {
	if len(os.Args) < 3 {
		return
	}
	cfg, err := parseRunConfig(os.Args[2:])
	if err != nil || len(cfg.Command) == 0 {
		return
	}
	commandStart := len(os.Args) - len(cfg.Command)
	if commandStart < 2 || commandStart > len(os.Args) {
		return
	}
	wrapped := make([]string, 0, len(os.Args)+2)
	wrapped = append(wrapped, os.Args[:commandStart]...)
	wrapped = append(wrapped, "/proc/self/exe", initSupervisorArg)
	wrapped = append(wrapped, cfg.Command...)
	os.Args = wrapped
}

func runContainerInitSupervisor(command []string) (int, error) {
	if len(command) == 0 || command[0] == "" {
		return 0, fmt.Errorf("payload command is empty")
	}

	// PID 1 already adopts orphaned descendants in its PID namespace. Marking it
	// as a child subreaper makes that contract explicit and also lets deterministic
	// integration tests exercise the same behavior without creating a PID namespace.
	if err := unix.Prctl(unix.PR_SET_CHILD_SUBREAPER, 1, 0, 0, 0); err != nil {
		return 0, fmt.Errorf("enable child subreaper: %w", err)
	}

	binary, err := exec.LookPath(command[0])
	if err != nil {
		return 0, fmt.Errorf("resolve payload executable %q: %w", command[0], err)
	}

	forwardedSignals := make(chan os.Signal, 16)
	signal.Notify(forwardedSignals, initSupervisorForwardSignals...)
	defer signal.Stop(forwardedSignals)

	pid, err := syscall.ForkExec(binary, command, &syscall.ProcAttr{
		Env:   os.Environ(),
		Files: []uintptr{os.Stdin.Fd(), os.Stdout.Fd(), os.Stderr.Fd()},
	})
	if err != nil {
		return 0, fmt.Errorf("start payload: %w", err)
	}

	doneForwarding := make(chan struct{})
	defer close(doneForwarding)
	forwardingErr := make(chan error, 1)
	go func() {
		for {
			select {
			case <-doneForwarding:
				return
			case sig := <-forwardedSignals:
				if sig == nil {
					continue
				}
				s, ok := sig.(syscall.Signal)
				if !ok {
					continue
				}
				if err := syscall.Kill(pid, s); err != nil && !errors.Is(err, syscall.ESRCH) {
					select {
					case forwardingErr <- fmt.Errorf("forward signal %v to payload %d: %w", sig, pid, err):
					default:
					}
				}
			}
		}
	}()

	for {
		var status syscall.WaitStatus
		reapedPID, err := syscall.Wait4(-1, &status, 0, nil)
		if err != nil {
			if errors.Is(err, syscall.EINTR) {
				runtime.Gosched()
				continue
			}
			return 0, fmt.Errorf("reap child process: %w", err)
		}
		if reapedPID != pid {
			// Reap orphaned descendants and continue supervising the primary payload.
			continue
		}

		select {
		case err := <-forwardingErr:
			return 0, err
		default:
		}
		if status.Exited() {
			return status.ExitStatus(), nil
		}
		if status.Signaled() {
			return 128 + int(status.Signal()), nil
		}
		return 1, fmt.Errorf("payload %d exited with unsupported wait status %#x", pid, uint32(status))
	}
}
