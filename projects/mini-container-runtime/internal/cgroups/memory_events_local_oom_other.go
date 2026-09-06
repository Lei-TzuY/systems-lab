//go:build !linux

package cgroups

func ReadLocalOOMCount(cgroupPath string) (uint64, error) {
	return 0, nil
}
