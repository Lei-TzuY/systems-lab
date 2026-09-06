//go:build !linux

package cgroups

func SetMemoryLow(cgroupPath string, lowBytes int64) error {
	return nil
}
