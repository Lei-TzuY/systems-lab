//go:build linux

package cgroups

import (
	"fmt"
	"os"
	"path/filepath"
)

// SetMemoryMin writes Cgroup v2 memory.min page protection rules.
func SetMemoryMin(cgroupPath string, minBytes int64) error {
	minFile := filepath.Join(cgroupPath, "memory.min")
	val := "max"
	if minBytes >= 0 {
		val = fmt.Sprintf("%d", minBytes)
	}
	return os.WriteFile(minFile, []byte(val+"\n"), 0644)
}
