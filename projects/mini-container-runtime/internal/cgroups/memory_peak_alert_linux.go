//go:build linux

package cgroups

import (
	"os"
	"path/filepath"
	"strconv"
	"strings"
)

// IsMemoryPeakHigh checks if memory.peak has reached thresholdRatio relative to memory.max.
func IsMemoryPeakHigh(cgroupPath string, thresholdRatio float64) (bool, error) {
	peakFile := filepath.Join(cgroupPath, "memory.peak")
	maxFile := filepath.Join(cgroupPath, "memory.max")

	peakBytes, err := os.ReadFile(peakFile)
	if err != nil {
		return false, nil
	}
	maxBytes, err := os.ReadFile(maxFile)
	if err != nil {
		return false, nil
	}

	peak, _ := strconv.ParseUint(strings.TrimSpace(string(peakBytes)), 10, 64)
	maxStr := strings.TrimSpace(string(maxBytes))
	if maxStr == "max" {
		return false, nil
	}
	maxVal, _ := strconv.ParseUint(maxStr, 10, 64)
	if maxVal == 0 {
		return false, nil
	}

	return (float64(peak) / float64(maxVal)) >= thresholdRatio, nil
}
