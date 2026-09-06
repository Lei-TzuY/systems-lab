//go:build !linux

package cgroups

func ApplyIOWeight(cgroupPath string, weight int) error {
	return nil
}
