//go:build !linux

package cgroups

func ApplyIOLatency(cgroupPath string, targetMs int) error {
	return nil
}
