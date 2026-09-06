package cgroups

import "errors"

// ErrCPUBurstUnavailable reports that the kernel/platform does not expose
// the cgroup v2 cpu.max.burst controller interface (requires Linux 5.14+).
var ErrCPUBurstUnavailable = errors.New("cgroup cpu.max.burst is unavailable")
