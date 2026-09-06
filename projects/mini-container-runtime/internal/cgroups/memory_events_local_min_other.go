//go:build !linux

package cgroups

func ReadLocalMemoryMinCount(cgroupPath string) (uint64, error) {
	return 0, nil
}
