package cgroups

import "errors"

// ErrMemoryReclaimUnavailable reports that the kernel/platform does not expose
// the cgroup v2 memory.reclaim interface (requires Linux 5.19+).
var ErrMemoryReclaimUnavailable = errors.New("cgroup memory.reclaim is unavailable")
