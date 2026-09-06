//go:build !linux

package cgroups

func ReadLocalMemoryLowCount(cgroupPath string) (uint64, error) {
	return 0, nil
}
