//go:build linux

package cgroups

import (
	"os"
	"path/filepath"
)

// SetMemoryOOMGroup enables/disables Cgroup v2 memory.oom.group atomic killer.
func SetMemoryOOMGroup(cgroupPath string, enable bool) error {
	oomGroupFile := filepath.Join(cgroupPath, "memory.oom.group")
	val := "1\n"
	if !enable {
		val = "0\n"
	}
	return os.WriteFile(oomGroupFile, []byte(val), 0644)
}
