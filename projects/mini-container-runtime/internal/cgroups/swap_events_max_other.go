//go:build !linux

package cgroups

func ReadSwapMaxCount(cgroupPath string) (uint64, error) {
	return 0, nil
}
