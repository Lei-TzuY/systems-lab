//go:build linux

package cgroups

import (
	"errors"
	"fmt"
	"os"
	"path/filepath"
)

// RemoveChecked removes a managed cgroup and reports cleanup failures instead
// of silently discarding them. Missing cgroups are treated as already clean.
func RemoveChecked(name string, debug bool) error {
	if err := validateCgroupName(name); err != nil {
		return err
	}

	if isV2() {
		return removeCgroupPaths([]string{filepath.Join(cgroupV2Root, name)}, debug)
	}

	paths := make([]string, 0, 3)
	for _, controller := range []string{"memory", "cpu", "pids"} {
		paths = append(paths, filepath.Join("/sys/fs/cgroup", controller, name))
	}
	return removeCgroupPaths(paths, debug)
}

func removeCgroupPaths(paths []string, debug bool) error {
	var cleanupErr error
	for _, path := range paths {
		if err := os.Remove(path); err != nil && !os.IsNotExist(err) {
			wrapped := fmt.Errorf("remove cgroup %s: %w", path, err)
			cleanupErr = errors.Join(cleanupErr, wrapped)
			if debug {
				fmt.Printf("[cgroup] cleanup %s: %v\n", path, err)
			}
		}
	}
	return cleanupErr
}
