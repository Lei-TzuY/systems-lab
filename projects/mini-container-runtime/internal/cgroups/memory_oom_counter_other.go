//go:build !linux

package cgroups

func ReadOOMCounter(cgroupPath string) (uint64, error) {
	return 0, nil
}
