//go:build linux

package cgroups

import (
	"fmt"
	"os"
	"path/filepath"
	"strconv"
	"strings"
)

// ReadCPUIdle reads the cpu.idle value from the cgroup directory.
// A value of 1 indicates SCHED_IDLE priority class; 0 is normal.
func ReadCPUIdle(cgroupPath string) (int, error) {
	data, err := os.ReadFile(filepath.Join(cgroupPath, "cpu.idle"))
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
	return strconv.Atoi(val)
}

// WriteCPUIdle sets the cpu.idle value in the cgroup directory.
// idle must be either 0 (normal scheduling) or 1 (SCHED_IDLE background priority).
func WriteCPUIdle(cgroupPath string, idle int) error {
	if idle != 0 && idle != 1 {
		return fmt.Errorf("invalid cpu.idle value %d (must be 0 or 1)", idle)
	}
	return os.WriteFile(filepath.Join(cgroupPath, "cpu.idle"), []byte(strconv.Itoa(idle)+"\n"), 0644)
}
