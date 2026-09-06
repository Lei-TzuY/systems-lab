//go:build linux

package cgroups

import (
	"fmt"
	"os"
	"path/filepath"
)

// ApplyIOWeight writes Cgroup v2 io.weight fair share priority.
func ApplyIOWeight(cgroupPath string, weight int) error {
	if weight <= 0 {
		weight = 100
	}
	weightFile := filepath.Join(cgroupPath, "io.weight")
	return os.WriteFile(weightFile, []byte(fmt.Sprintf("default %d\n", weight)), 0644)
}
