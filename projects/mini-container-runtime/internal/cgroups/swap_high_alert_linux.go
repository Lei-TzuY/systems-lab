//go:build linux

package cgroups

import (
	"os"
	"path/filepath"
	"strconv"
	"strings"
)

// IsSwapHighExceeded checks if current swap usage exceeds memory.swap.high soft limit.
func IsSwapHighExceeded(cgroupPath string) (bool, error) {
	currFile := filepath.Join(cgroupPath, "memory.swap.current")
	highFile := filepath.Join(cgroupPath, "memory.swap.high")

	currBytes, err := os.ReadFile(currFile)
	if err != nil {
		return false, nil
	}
	highBytes, err := os.ReadFile(highFile)
	if err != nil {
		return false, nil
	}

	curr, _ := strconv.ParseUint(strings.TrimSpace(string(currBytes)), 10, 64)
	highStr := strings.TrimSpace(string(highBytes))
	if highStr == "max" {
		return false, nil
	}
	high, _ := strconv.ParseUint(highStr, 10, 64)

	return curr > high && high > 0, nil
}
