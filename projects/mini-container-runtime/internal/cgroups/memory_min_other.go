//go:build !linux

package cgroups

func SetMemoryMin(cgroupPath string, minBytes int64) error {
	return nil
}
