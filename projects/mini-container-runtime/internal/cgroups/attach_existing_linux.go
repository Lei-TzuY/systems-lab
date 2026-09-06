//go:build linux

package cgroups

import (
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"strconv"
)

// AttachExisting moves pid into an already-created minicontainer cgroup.
// It never creates or reconfigures the cgroup: exec processes must inherit the
// resource domain owned by the running container rather than mutate limits.
func AttachExisting(pid int, name string, debug bool) error {
	if pid <= 0 {
		return fmt.Errorf("invalid cgroup target PID %d", pid)
	}
	if err := validateCgroupName(name); err != nil {
		return err
	}
	if isV2() {
		return attachExistingV2At(cgroupV2Root, pid, name, debug)
	}
	return attachExistingV1At("/sys/fs/cgroup", pid, name, debug)
}

func attachExistingV2At(root string, pid int, name string, debug bool) error {
	path := filepath.Join(root, name, "cgroup.procs")
	if err := os.WriteFile(path, []byte(strconv.Itoa(pid)), 0o644); err != nil {
		return fmt.Errorf("attach PID %d to cgroup v2 %s: %w", pid, path, err)
	}
	if debug {
		fmt.Printf("[cgroup v2] existing %s/cgroup.procs = %d\n", name, pid)
	}
	return nil
}

func attachExistingV1At(root string, pid int, name string, debug bool) error {
	pidStr := strconv.Itoa(pid)
	attached := make([]string, 0, 3)
	found := false
	for _, controller := range []string{"memory", "cpu", "pids"} {
		cgPath := filepath.Join(root, controller, name)
		if _, err := os.Stat(cgPath); errors.Is(err, os.ErrNotExist) {
			continue
		} else if err != nil {
			return fmt.Errorf("inspect cgroup v1 %s: %w", cgPath, err)
		}
		found = true
		tasks := filepath.Join(cgPath, "tasks")
		if err := os.WriteFile(tasks, []byte(pidStr), 0o644); err != nil {
			attachErr := fmt.Errorf("attach PID %d to cgroup v1 %s: %w", pid, tasks, err)
			return rollbackV1Attachments(root, pidStr, attached, attachErr, debug)
		}
		attached = append(attached, controller)
		if debug {
			fmt.Printf("[cgroup v1] existing %s/tasks = %s\n", controller, pidStr)
		}
	}
	if !found {
		return fmt.Errorf("existing cgroup %q was not found in any v1 controller", name)
	}
	return nil
}
