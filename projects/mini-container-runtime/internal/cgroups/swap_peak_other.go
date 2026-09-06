//go:build !linux

package cgroups

func ReadSwapPeak(cgroupPath string) (uint64, error) {
	return 0, nil
}
