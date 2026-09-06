//go:build linux

package cgroups

import (
	"fmt"
	"os"
	"path/filepath"
	"strconv"
	"strings"
)

// ReadMemoryZswapMax reads the memory.zswap.max limit from the cgroup directory.
// Returns "max" or the numeric byte limit as a string.
func ReadMemoryZswapMax(cgroupPath string) (string, error) {
	data, err := os.ReadFile(filepath.Join(cgroupPath, "memory.zswap.max"))
	if err != nil {
		if os.IsNotExist(err) {
			return "max", nil
		}
		return "", err
	}
	val := strings.TrimSpace(string(data))
	if val == "" {
		return "max", nil
	}
	return val, nil
}

// WriteMemoryZswapMax sets the memory.zswap.max limit in the cgroup directory.
// limit can be "max" or a positive byte count.
func WriteMemoryZswapMax(cgroupPath string, limit string) error {
	limit = strings.TrimSpace(limit)
	if limit == "" {
		return fmt.Errorf("zswap.max limit cannot be empty")
	}
	if limit != "max" {
		if _, err := strconv.ParseUint(limit, 10, 64); err != nil {
			return fmt.Errorf("invalid zswap.max value %q: %w", limit, err)
		}
	}
	return os.WriteFile(filepath.Join(cgroupPath, "memory.zswap.max"), []byte(limit+"\n"), 0644)
}
