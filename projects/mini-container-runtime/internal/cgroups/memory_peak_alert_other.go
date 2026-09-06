//go:build !linux

package cgroups

func IsMemoryPeakHigh(cgroupPath string, thresholdRatio float64) (bool, error) {
	return false, nil
}
