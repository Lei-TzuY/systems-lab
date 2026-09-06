//go:build !linux

package cgroups

func ReadLocalOOMKillCount(cgroupPath string) (uint64, error) {
	return 0, nil
}
