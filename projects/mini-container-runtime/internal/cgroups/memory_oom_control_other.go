//go:build !linux

package cgroups

func SetMemoryOOMGroup(cgroupPath string, enable bool) error {
	return nil
}
