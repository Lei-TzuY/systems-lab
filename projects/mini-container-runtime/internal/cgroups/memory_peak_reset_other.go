//go:build !linux

package cgroups

func ResetMemoryPeak(cgroupPath string) error {
	return nil
}
