//go:build !linux

package cgroups

// MemoryReclaimOptions holds advanced arguments for Cgroup v2 memory.reclaim.
type MemoryReclaimOptions struct {
	BytesToReclaim int64
	Swappiness     int
	NumaNode       int
}

// ReclaimMemoryWithOptions reports unsupported interface on non-Linux platforms.
func ReclaimMemoryWithOptions(cgroupPath string, opts MemoryReclaimOptions) error {
	return ErrMemoryReclaimUnavailable
}
