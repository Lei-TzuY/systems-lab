//go:build linux

package cgroups

import (
	"os"
	"path/filepath"
	"strconv"
	"strings"
)

// ReadMemoryZswapCurrent reads the memory.zswap.current value from the cgroup directory.
// This interface reports the amount of memory in bytes used by zswap compression for the cgroup.
func ReadMemoryZswapCurrent(cgroupPath string) (uint64, error) {
	data, err := os.ReadFile(filepath.Join(cgroupPath, "memory.zswap.current"))
	if err != nil {
		if os.IsNotExist(err) {
			return 0, nil
		}
		return 0, err
	}
	val := strings.TrimSpace(string(data))
	if val == "" {
		return 0, nil
	}
	return strconv.ParseUint(val, 10, 64)
}
