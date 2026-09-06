//go:build !linux

package cgroups

func ReadOOMKillCount(cgroupPath string) (uint64, error) {
	return 0, nil
}
