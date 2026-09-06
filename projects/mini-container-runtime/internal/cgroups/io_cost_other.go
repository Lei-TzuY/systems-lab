//go:build !linux

package cgroups

func ApplyIOCost(cgroupPath string, enable bool) error {
	return nil
}
