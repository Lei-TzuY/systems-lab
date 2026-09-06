//go:build linux

package cgroups

import (
	"os"
	"path/filepath"
	"strconv"
	"strings"
)

// ReadCPUWeight reads the cpu.weight value from the cgroup directory.
// cpu.weight controls the relative CPU time share for the cgroup
// (range 1–10000, default 100). Returns 0 if the file is missing.
func ReadCPUWeight(cgroupPath string) (uint64, error) {
	data, err := os.ReadFile(filepath.Join(cgroupPath, "cpu.weight"))
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
	return strconv.ParseUint(val, 10, 64)
}
