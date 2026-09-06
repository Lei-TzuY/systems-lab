//go:build linux

package cgroups

import (
	"fmt"
	"os"
	"path/filepath"
)

// ApplyCPUWeight writes Cgroup v2 cpu.weight fair-share priority.
func ApplyCPUWeight(cgroupPath string, weight int) error {
	if weight <= 0 {
		weight = 100
	}
	weightFile := filepath.Join(cgroupPath, "cpu.weight")
	return os.WriteFile(weightFile, []byte(fmt.Sprintf("%d\n", weight)), 0644)
}

// ApplyCPUMax writes Cgroup v2 cpu.max bandwidth quota and period.
func ApplyCPUMax(cgroupPath string, quotaUs int64, periodUs int64) error {
	if periodUs <= 0 {
		periodUs = 100000
	}
	maxFile := filepath.Join(cgroupPath, "cpu.max")
	val := "max"
	if quotaUs > 0 {
		val = fmt.Sprintf("%d %d", quotaUs, periodUs)
	} else {
		val = fmt.Sprintf("max %d", periodUs)
	}
	return os.WriteFile(maxFile, []byte(val+"\n"), 0644)
}
