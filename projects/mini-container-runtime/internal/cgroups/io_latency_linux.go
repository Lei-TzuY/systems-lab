//go:build linux

package cgroups

import (
	"fmt"
	"os"
	"path/filepath"
)

// ApplyIOLatency writes Cgroup v2 io.latency target protection rules.
func ApplyIOLatency(cgroupPath string, targetMs int) error {
	if targetMs <= 0 {
		targetMs = 10
	}
	latFile := filepath.Join(cgroupPath, "io.latency")
	return os.WriteFile(latFile, []byte(fmt.Sprintf("target=%d\n", targetMs)), 0644)
}
