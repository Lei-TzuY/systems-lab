//go:build !linux

package cgroups

func ApplySwapHigh(cgroupPath string, softSwapBytes int64) error {
	return nil
}
