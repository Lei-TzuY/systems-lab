//go:build !linux

package cgroups

// ReadMemorySwapCurrent reports unsupported telemetry on non-Linux platforms.
func ReadMemorySwapCurrent(cgroupPath string) (uint64, error) {
	return 0, ErrMemorySwapUnavailable
}

// ReadMemorySwapHigh reports unsupported telemetry on non-Linux platforms.
func ReadMemorySwapHigh(cgroupPath string) (uint64, bool, error) {
	return 0, false, ErrMemorySwapUnavailable
}

// WriteMemorySwapHigh reports unsupported controller on non-Linux platforms.
func WriteMemorySwapHigh(cgroupPath string, limitBytes int64) error {
	return ErrMemorySwapUnavailable
}
