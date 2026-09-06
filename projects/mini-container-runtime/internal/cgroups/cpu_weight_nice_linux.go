//go:build linux

package cgroups

import (
	"fmt"
	"os"
	"path/filepath"
	"strconv"
	"strings"
)

// ReadCPUWeightNice reads the cpu.weight.nice value from the cgroup directory.
// This maps to traditional nice values (-20 to 19) translated from cgroup v2 cpu.weight.
func ReadCPUWeightNice(cgroupPath string) (int, error) {
	data, err := os.ReadFile(filepath.Join(cgroupPath, "cpu.weight.nice"))
	if err != nil {
		if os.IsNotExist(err) {
			return 0, nil // default nice 0
		}
		return 0, err
	}
	val := strings.TrimSpace(string(data))
	if val == "" {
		return 0, nil
	}
	return strconv.Atoi(val)
}

// WriteCPUWeightNice writes a nice value (-20 to 19) to cpu.weight.nice.
func WriteCPUWeightNice(cgroupPath string, nice int) error {
	if nice < -20 || nice > 19 {
		return fmt.Errorf("invalid nice value %d (must be -20 to 19)", nice)
	}
	return os.WriteFile(filepath.Join(cgroupPath, "cpu.weight.nice"),
		[]byte(strconv.Itoa(nice)+"\n"), 0644)
}
