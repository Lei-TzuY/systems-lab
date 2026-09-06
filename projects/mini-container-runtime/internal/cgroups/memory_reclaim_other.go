//go:build !linux

package cgroups

func ReclaimMemory(cgroupPath string, bytesToReclaim int64) error {
	return nil
}
