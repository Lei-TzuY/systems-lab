//go:build linux

package cgroups

import (
	"os"
	"path/filepath"
)

// ResetMemoryPeak resets the Cgroup v2 memory peak watermark where supported.
func ResetMemoryPeak(cgroupPath string) error {
	reclaimFile := filepath.Join(cgroupPath, "memory.reclaim")
	return os.WriteFile(reclaimFile, []byte("0\n"), 0644)
}
