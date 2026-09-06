//go:build !linux

package cgroups

func ReadMemoryMinCounter(cgroupPath string) (uint64, error) {
	return 0, nil
}
