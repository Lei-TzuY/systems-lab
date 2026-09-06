package cgroups

import "errors"

// ErrMemorySwapUnavailable reports that the kernel/platform does not expose
// the cgroup v2 memory.swap.* controller interfaces (swap controller not enabled).
var ErrMemorySwapUnavailable = errors.New("cgroup memory swap interface is unavailable")
