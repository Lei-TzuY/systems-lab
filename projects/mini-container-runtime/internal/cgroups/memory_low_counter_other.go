//go:build !linux

package cgroups

func ReadMemoryLowCounter(cgroupPath string) (uint64, error) {
	return 0, nil
}
