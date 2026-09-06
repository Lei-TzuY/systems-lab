//go:build !linux

package cgroups

func ApplyMemoryAdvanced(cgroupPath string, reservationBytes int64, swapBytes int64) error {
	return nil
}
