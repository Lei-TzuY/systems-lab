//go:build !linux

package cgroups

func ReadMemoryHighCounter(cgroupPath string) (uint64, error) {
	return 0, nil
}
