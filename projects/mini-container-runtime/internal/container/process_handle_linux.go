//go:build linux

package container

import (
	"errors"
	"fmt"
	"os"
	"syscall"
	"time"

	"golang.org/x/sys/unix"
)

// ProcessHandle is a stable kernel reference to one Linux process. Once opened,
// the pidfd continues to identify that process even if its numeric PID is later
// recycled for an unrelated process.
type ProcessHandle struct {
	fd        int
	PID       int
	StartTime uint64
}

// OpenProcessHandle opens a pidfd first, then verifies the persisted Linux
// process starttime. Opening before verification closes the classic
// check-starttime-then-kill PID reuse race.
func OpenProcessHandle(pid int, expectedStartTime uint64) (*ProcessHandle, error) {
	if pid <= 0 || expectedStartTime == 0 {
		return nil, fmt.Errorf("%w: invalid identity %d/%d", ErrProcessIdentityUnavailable, pid, expectedStartTime)
	}

	fd, err := unix.PidfdOpen(pid, 0)
	if err != nil {
		if errors.Is(err, unix.ESRCH) {
			return nil, fmt.Errorf("%w: PID %d", ErrProcessNotFound, pid)
		}
		return nil, fmt.Errorf("pidfd_open PID %d: %w", pid, err)
	}

	start, err := ProcessStartTime(pid)
	if err != nil {
		_ = unix.Close(fd)
		if errors.Is(err, os.ErrNotExist) || errors.Is(err, unix.ENOENT) {
			return nil, fmt.Errorf("%w: PID %d exited during identity verification", ErrProcessNotFound, pid)
		}
		return nil, fmt.Errorf("verify PID %d starttime: %w", pid, err)
	}
	if start != expectedStartTime {
		_ = unix.Close(fd)
		return nil, fmt.Errorf("%w: PID %d expected starttime %d, got %d", ErrProcessIdentityMismatch, pid, expectedStartTime, start)
	}

	return &ProcessHandle{fd: fd, PID: pid, StartTime: start}, nil
}

// Signal sends sig through pidfd_send_signal to this exact process identity.
func (h *ProcessHandle) Signal(sig syscall.Signal) error {
	if h == nil || h.fd < 0 {
		return fmt.Errorf("invalid process handle")
	}
	if sig <= 0 {
		return fmt.Errorf("invalid signal %d", sig)
	}
	if err := unix.PidfdSendSignal(h.fd, unix.Signal(sig), nil, 0); err != nil {
		if errors.Is(err, unix.ESRCH) {
			return fmt.Errorf("%w: PID %d", ErrProcessNotFound, h.PID)
		}
		return fmt.Errorf("pidfd_send_signal PID %d signal %d: %w", h.PID, sig, err)
	}
	return nil
}

// WaitExit waits until the process referenced by the pidfd exits. false, nil
// means the timeout elapsed while the same process was still alive.
func (h *ProcessHandle) WaitExit(timeout time.Duration) (bool, error) {
	if h == nil || h.fd < 0 {
		return false, fmt.Errorf("invalid process handle")
	}
	if timeout < 0 {
		return false, fmt.Errorf("wait timeout must not be negative")
	}

	deadline := time.Now().Add(timeout)
	for {
		remaining := time.Until(deadline)
		if timeout == 0 {
			remaining = 0
		}
		if remaining < 0 {
			return false, nil
		}
		ms := int((remaining + time.Millisecond - 1) / time.Millisecond)
		fds := []unix.PollFd{{Fd: int32(h.fd), Events: unix.POLLIN}}
		n, err := unix.Poll(fds, ms)
		if err != nil {
			if errors.Is(err, unix.EINTR) {
				continue
			}
			return false, fmt.Errorf("poll pidfd for PID %d: %w", h.PID, err)
		}
		if n == 0 {
			return false, nil
		}
		if fds[0].Revents&(unix.POLLIN|unix.POLLHUP|unix.POLLERR) != 0 {
			return true, nil
		}
	}
}

func (h *ProcessHandle) Close() error {
	if h == nil || h.fd < 0 {
		return nil
	}
	fd := h.fd
	h.fd = -1
	if err := unix.Close(fd); err != nil && !errors.Is(err, unix.EBADF) {
		return fmt.Errorf("close pidfd for PID %d: %w", h.PID, err)
	}
	return nil
}
