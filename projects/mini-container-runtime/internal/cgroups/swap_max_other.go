//go:build !linux

package cgroups

func ApplySwapMax(cgroupPath string, maxSwapBytes int64) error {
	return nil
}
