//go:build !linux

package cgroups

func ApplyCPUWeight(cgroupPath string, weight int) error {
	return nil
}

func ApplyCPUMax(cgroupPath string, quotaUs int64, periodUs int64) error {
	return nil
}
