//go:build !linux

package cgroups

func IsSwapHighExceeded(cgroupPath string) (bool, error) {
	return false, nil
}
