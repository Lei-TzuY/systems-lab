//go:build !linux

package container

import (
	"fmt"
	"syscall"
	"time"
)

type ProcessHandle struct {
	PID       int
	StartTime uint64
}

func OpenProcessHandle(pid int, expectedStartTime uint64) (*ProcessHandle, error) {
	return nil, fmt.Errorf("%w: pidfd process handles require Linux", ErrProcessControlUnsupported)
}

func (h *ProcessHandle) Signal(sig syscall.Signal) error {
	return fmt.Errorf("%w: pidfd process handles require Linux", ErrProcessControlUnsupported)
}

func (h *ProcessHandle) WaitExit(timeout time.Duration) (bool, error) {
	return false, fmt.Errorf("%w: pidfd process handles require Linux", ErrProcessControlUnsupported)
}

func (h *ProcessHandle) Close() error { return nil }
