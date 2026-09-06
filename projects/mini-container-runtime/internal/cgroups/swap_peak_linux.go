//go:build linux

package cgroups

import (
	"os"
	"path/filepath"
	"strconv"
	"strings"
)

// ReadSwapPeak reads Cgroup v2 memory.swap.peak usage watermark.
func ReadSwapPeak(cgroupPath string) (uint64, error) {
	peakFile := filepath.Join(cgroupPath, "memory.swap.peak")
	data, err := os.ReadFile(peakFile)
	if err != nil {
		return 0, err
	}

	valStr := strings.TrimSpace(string(data))
	return strconv.ParseUint(valStr, 10, 64)
}
