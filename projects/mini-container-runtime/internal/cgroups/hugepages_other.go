//go:build !linux

package cgroups

func ApplyHugeTLBLimit(cgroupPath string, pageSize string, limitBytes int64) error {
	return nil
}
