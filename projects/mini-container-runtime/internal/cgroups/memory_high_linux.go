//go:build linux

package cgroups

import (
	"fmt"
	"os"
	"path/filepath"
)

// ApplyMemoryHigh writes Cgroup v2 memory.high soft limit.
func ApplyMemoryHigh(cgroupPath string, softLimitBytes int64) error {
	highFile := filepath.Join(cgroupPath, "memory.high")
	val := "max"
	if softLimitBytes > 0 {
		val = fmt.Sprintf("%d", softLimitBytes)
	}
	return os.WriteFile(highFile, []byte(val+"\n"), 0644)
}
