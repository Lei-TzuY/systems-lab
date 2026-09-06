//go:build linux

package cgroups

import (
	"fmt"
	"os"
	"path/filepath"
)

// SetMemoryLow writes Cgroup v2 memory.low soft protection watermarks.
func SetMemoryLow(cgroupPath string, lowBytes int64) error {
	lowFile := filepath.Join(cgroupPath, "memory.low")
	val := "max"
	if lowBytes >= 0 {
		val = fmt.Sprintf("%d", lowBytes)
	}
	return os.WriteFile(lowFile, []byte(val+"\n"), 0644)
}
