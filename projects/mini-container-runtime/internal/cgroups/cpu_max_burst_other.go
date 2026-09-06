//go:build !linux

package cgroups

// ReadCPUMaxBurst reports unsupported telemetry on non-Linux platforms.
func ReadCPUMaxBurst(cgroupPath string) (uint64, error) {
	return 0, ErrCPUBurstUnavailable
}

// WriteCPUMaxBurst reports unsupported controller on non-Linux platforms.
func WriteCPUMaxBurst(cgroupPath string, burstUsec uint64) error {
	return ErrCPUBurstUnavailable
}
