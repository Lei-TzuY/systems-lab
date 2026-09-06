//go:build linux

package cgroups

import (
	"fmt"
	"os"
	"path/filepath"
)

// ApplySwapMax writes Cgroup v2 memory.swap.max hard swap limit rules.
func ApplySwapMax(cgroupPath string, maxSwapBytes int64) error {
	swapFile := filepath.Join(cgroupPath, "memory.swap.max")
	val := "max"
	if maxSwapBytes >= 0 {
		val = fmt.Sprintf("%d", maxSwapBytes)
	}
	return os.WriteFile(swapFile, []byte(val+"\n"), 0644)
}
