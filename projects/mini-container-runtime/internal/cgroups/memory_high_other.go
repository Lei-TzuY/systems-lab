//go:build !linux

package cgroups

func ApplyMemoryHigh(cgroupPath string, softLimitBytes int64) error {
	return nil
}
