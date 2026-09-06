//go:build linux

// internal/cgroups/update_linux.go
//
// Dynamic Container Resource Updating (`minictl update`)
// ───────────────────────────────────────────────────────
// Dynamically adjusts running container cgroup v2 limits (memory, CPUs, CPU weight, PIDs limit)
// without stopping or restarting the container process.

package cgroups

import (
	"fmt"
	"os"
	"path/filepath"
)

// UpdateConfig holds resource limits to update dynamically.
type UpdateConfig struct {
	MemoryMax int64
	CPUs      float64
	CPUWeight int64
	PidsMax   int64
}

// UpdateLimits dynamically modifies cgroup limits for a running container. The
// multi-file update is rollback-capable; callers that may race with another
// process must additionally serialize access to the target cgroup generation.
func UpdateLimits(cgroupName string, cfg UpdateConfig, debug bool) error {
	if err := validateCgroupName(cgroupName); err != nil {
		return err
	}
	if err := validateResourceValues(cfg.MemoryMax, cfg.CPUWeight, cfg.CPUs, cfg.PidsMax); err != nil {
		return err
	}

	cgPath := filepath.Join(cgroupV2Root, cgroupName)
	info, err := os.Stat(cgPath)
	if err != nil {
		if os.IsNotExist(err) {
			return fmt.Errorf("cgroup %s does not exist", cgroupName)
		}
		return fmt.Errorf("stat cgroup %s: %w", cgroupName, err)
	}
	if !info.IsDir() {
		return fmt.Errorf("cgroup path %s is not a directory", cgPath)
	}

	return applyResourceUpdateTransaction(cgPath, cfg, debug, updateFileOps{
		readFile:  os.ReadFile,
		writeFile: os.WriteFile,
	})
}
