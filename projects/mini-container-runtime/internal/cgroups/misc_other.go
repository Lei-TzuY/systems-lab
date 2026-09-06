//go:build !linux

package cgroups

func ApplyMiscLimit(cgroupPath string, resource string, limit int64) error {
	return nil
}
