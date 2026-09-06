//go:build !linux

package cgroups

// ReadPIDSPeak reports unsupported telemetry on non-Linux platforms.
func ReadPIDSPeak(cgroupPath string) (uint64, error) {
	return 0, ErrPIDSPeakUnavailable
}

// ResetPIDSPeak is retained for source compatibility. pids.peak is a Linux
// cgroup v2 read-only telemetry file and cannot be reset.
func ResetPIDSPeak(cgroupPath string) error {
	return ErrPIDSPeakReadOnly
}
