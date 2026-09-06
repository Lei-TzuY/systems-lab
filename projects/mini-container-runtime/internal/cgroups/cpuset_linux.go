//go:build linux

package cgroups

import (
	"os"
	"path/filepath"
	"strings"
)

// ApplyCPUSet configures Cgroup v2 cpuset.cpus and cpuset.mems.
func ApplyCPUSet(cgroupPath string, cpus string, mems string) error {
	cpus = strings.TrimSpace(cpus)
	if cpus != "" {
		cpusFile := filepath.Join(cgroupPath, "cpuset.cpus")
		if err := os.WriteFile(cpusFile, []byte(cpus+"\n"), 0644); err != nil {
			return err
		}
	}

	mems = strings.TrimSpace(mems)
	if mems != "" {
		memsFile := filepath.Join(cgroupPath, "cpuset.mems")
		if err := os.WriteFile(memsFile, []byte(mems+"\n"), 0644); err != nil {
			return err
		}
	}

	return nil
}
