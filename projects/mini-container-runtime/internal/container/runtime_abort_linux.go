//go:build linux

package container

import (
	"errors"
	"fmt"
	"os"
	"os/exec"
)

type blockedChildAborter func(cmd *exec.Cmd, writePipe *os.File) (reaped bool, err error)

// abortBlockedChildChecked closes the parent's readiness writer, terminates the
// blocked child, and only reports reaped=true after Wait has authoritatively
// consumed the child process. Callers must not tear down process-owned resources
// or persist stopped lifecycle state unless reaped is true.
func abortBlockedChildChecked(cmd *exec.Cmd, writePipe *os.File) (bool, error) {
	if cmd == nil {
		return false, fmt.Errorf("blocked child command is nil")
	}
	if cmd.Process == nil {
		return false, fmt.Errorf("blocked child process is nil")
	}

	var closeSync func() error
	if writePipe != nil {
		closeSync = writePipe.Close
	}
	return abortBlockedChildWithOps(closeSync, cmd.Process.Kill, cmd.Wait)
}

func abortBlockedChildWithOps(closeSync, kill, wait func() error) (bool, error) {
	if kill == nil {
		return false, fmt.Errorf("blocked child kill operation is nil")
	}
	if wait == nil {
		return false, fmt.Errorf("blocked child wait operation is nil")
	}

	var resultErr error
	if closeSync != nil {
		if err := closeSync(); err != nil {
			resultErr = errors.Join(resultErr, fmt.Errorf("close blocked child readiness pipe: %w", err))
		}
	}

	if err := kill(); err != nil && !errors.Is(err, os.ErrProcessDone) {
		// Do not enter an unbounded Wait after a genuine kill failure. Without
		// proof that termination was initiated, the child may still be alive.
		return false, errors.Join(resultErr, fmt.Errorf("kill blocked child: %w", err))
	}

	waitErr := wait()
	if waitErr == nil {
		return true, resultErr
	}
	var exitErr *exec.ExitError
	if errors.As(waitErr, &exitErr) {
		// A regular non-zero/signal process status is exactly what abort expects;
		// Wait returning it still proves that the OS child was reaped.
		return true, resultErr
	}
	return false, errors.Join(resultErr, fmt.Errorf("reap blocked child: %w", waitErr))
}
