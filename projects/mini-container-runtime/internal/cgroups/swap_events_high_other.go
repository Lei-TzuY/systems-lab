//go:build !linux

package cgroups

func ReadSwapHighCount(cgroupPath string) (uint64, error) {
	return 0, nil
}
