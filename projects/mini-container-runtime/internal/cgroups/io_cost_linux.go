//go:build linux

package cgroups

import (
	"os"
	"path/filepath"
)

// ApplyIOCost writes Cgroup v2 io.cost.qos disk QoS cost rules.
func ApplyIOCost(cgroupPath string, enable bool) error {
	costFile := filepath.Join(cgroupPath, "io.cost.qos")
	val := "enable=1"
	if !enable {
		val = "enable=0"
	}
	return os.WriteFile(costFile, []byte(val+"\n"), 0644)
}
