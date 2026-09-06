//go:build linux

package cgroups

import (
	"fmt"
	"os"
	"path/filepath"
)

// ReclaimMemory triggers Cgroup v2 memory.reclaim page compacting.
func ReclaimMemory(cgroupPath string, bytesToReclaim int64) error {
	if bytesToReclaim <= 0 {
		bytesToReclaim = 1048576
	}

	reclaimFile := filepath.Join(cgroupPath, "memory.reclaim")
	return os.WriteFile(reclaimFile, []byte(fmt.Sprintf("%d\n", bytesToReclaim)), 0644)
}
