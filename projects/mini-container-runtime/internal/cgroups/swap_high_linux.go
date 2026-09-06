//go:build linux

package cgroups

import (
	"fmt"
	"os"
	"path/filepath"
)

// ApplySwapHigh writes Cgroup v2 memory.swap.high soft limit rules.
func ApplySwapHigh(cgroupPath string, softSwapBytes int64) error {
	swapFile := filepath.Join(cgroupPath, "memory.swap.high")
	val := "max"
	if softSwapBytes >= 0 {
		val = fmt.Sprintf("%d", softSwapBytes)
	}
	return os.WriteFile(swapFile, []byte(val+"\n"), 0644)
}
