//go:build linux

package cgroups

import (
	"fmt"
	"os"
	"path/filepath"
)

// ApplyMemoryAdvanced writes soft memory limit (memory.high) and swap limit (memory.swap.max).
func ApplyMemoryAdvanced(cgroupPath string, reservationBytes int64, swapBytes int64) error {
	if reservationBytes > 0 {
		highFile := filepath.Join(cgroupPath, "memory.high")
		_ = os.WriteFile(highFile, []byte(fmt.Sprintf("%d\n", reservationBytes)), 0644)
	}

	if swapBytes > 0 {
		swapFile := filepath.Join(cgroupPath, "memory.swap.max")
		_ = os.WriteFile(swapFile, []byte(fmt.Sprintf("%d\n", swapBytes)), 0644)
	}

	return nil
}
