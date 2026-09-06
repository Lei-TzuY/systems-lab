//go:build linux

package cgroups

import (
	"fmt"
	"os"
	"path/filepath"
	"strconv"
	"strings"
)

// ReadMemoryOOMGroup reads the memory.oom.group value from the cgroup directory.
// 1 indicates all processes in the cgroup are killed upon OOM; 0 indicates individual process kill.
func ReadMemoryOOMGroup(cgroupPath string) (int, error) {
	data, err := os.ReadFile(filepath.Join(cgroupPath, "memory.oom.group"))
	if err != nil {
		if os.IsNotExist(err) {
			return 0, nil // default disabled in kernel
		}
		return 0, err
	}
	val := strings.TrimSpace(string(data))
	if val == "" {
		return 0, nil
	}
	return strconv.Atoi(val)
}

// WriteMemoryOOMGroup sets the memory.oom.group value in the cgroup directory.
// enabled must be either 0 (kill individual process) or 1 (kill all cgroup processes on OOM).
func WriteMemoryOOMGroup(cgroupPath string, enabled int) error {
	if enabled != 0 && enabled != 1 {
		return fmt.Errorf("invalid memory.oom.group value %d (must be 0 or 1)", enabled)
	}
	return os.WriteFile(filepath.Join(cgroupPath, "memory.oom.group"), []byte(strconv.Itoa(enabled)+"\n"), 0644)
}
