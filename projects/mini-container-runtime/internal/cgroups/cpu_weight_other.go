//go:build !linux

package cgroups

func ReadCPUWeight(cgroupPath string) (uint64, error) {
	return 0, nil
}
