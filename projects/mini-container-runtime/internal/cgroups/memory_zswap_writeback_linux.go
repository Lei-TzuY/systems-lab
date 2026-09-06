//go:build linux

package cgroups

import (
	"fmt"
	"os"
	"path/filepath"
	"strconv"
	"strings"
)

// ReadMemoryZswapWriteback reads the memory.zswap.writeback value from the cgroup directory.
// 1 indicates writeback to disk swap is enabled; 0 indicates disabled.
func ReadMemoryZswapWriteback(cgroupPath string) (int, error) {
	data, err := os.ReadFile(filepath.Join(cgroupPath, "memory.zswap.writeback"))
	if err != nil {
		if os.IsNotExist(err) {
			return 1, nil // enabled by default in kernel
		}
		return 0, err
	}
	val := strings.TrimSpace(string(data))
	if val == "" {
		return 1, nil
	}
	return strconv.Atoi(val)
}

// WriteMemoryZswapWriteback sets the memory.zswap.writeback value in the cgroup directory.
// enabled must be either 0 (disable disk writeback) or 1 (enable disk writeback).
func WriteMemoryZswapWriteback(cgroupPath string, enabled int) error {
	if enabled != 0 && enabled != 1 {
		return fmt.Errorf("invalid memory.zswap.writeback value %d (must be 0 or 1)", enabled)
	}
	return os.WriteFile(filepath.Join(cgroupPath, "memory.zswap.writeback"), []byte(strconv.Itoa(enabled)+"\n"), 0644)
}
