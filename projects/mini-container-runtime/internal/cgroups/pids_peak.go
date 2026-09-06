package cgroups

import "errors"

var (
	// ErrPIDSPeakUnavailable reports that the kernel/platform does not expose
	// the cgroup v2 pids.peak read-only telemetry file.
	ErrPIDSPeakUnavailable = errors.New("cgroup pids.peak is unavailable")

	// ErrPIDSPeakReadOnly reports attempts to reset pids.peak. The cgroup v2
	// kernel ABI exposes pids.peak as read-only telemetry, so it cannot be reset.
	ErrPIDSPeakReadOnly = errors.New("cgroup pids.peak is read-only")
)
