//go:build !linux

package cgroups

func ApplyCPUSet(cgroupPath string, cpus string, mems string) error {
	return nil
}
