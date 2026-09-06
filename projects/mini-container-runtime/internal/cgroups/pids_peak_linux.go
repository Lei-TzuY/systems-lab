//go:build linux

package cgroups

import (
	"fmt"
	"os"
	"path/filepath"
	"strconv"
	"strings"
)

// ReadPIDSPeak reads the maximum number of concurrent tasks recorded in the
// cgroup v2 read-only pids.peak telemetry file.
func ReadPIDSPeak(cgroupPath string) (uint64, error) {
	peakFile := filepath.Join(cgroupPath, "pids.peak")
	data, err := os.ReadFile(peakFile)
	if err != nil {
		if os.IsNotExist(err) {
			return 0, fmt.Errorf("%w: %s", ErrPIDSPeakUnavailable, peakFile)
		}
		return 0, fmt.Errorf("read pids.peak: %w", err)
	}

	val := strings.TrimSpace(string(data))
	if val == "" {
		return 0, fmt.Errorf("parse pids.peak: empty value")
	}

	peak, err := strconv.ParseUint(val, 10, 64)
	if err != nil {
		return 0, fmt.Errorf("parse pids.peak value %q: %w", val, err)
	}
	return peak, nil
}

// ResetPIDSPeak is retained for source compatibility. The Linux cgroup v2 ABI
// defines pids.peak as read-only, so resetting it is not supported.
func ResetPIDSPeak(cgroupPath string) error {
	return ErrPIDSPeakReadOnly
}
