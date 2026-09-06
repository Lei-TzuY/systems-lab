//go:build linux

package cgroups

import (
	"fmt"
	"os"
	"path/filepath"
	"strconv"
	"strings"
)

// ProcessCleanup captures only cgroup paths that the exact target process was
// observed to belong to. Capture it before terminating the process, then remove
// it after the process has been reaped. This prevents cleanup from claiming a
// same-named cgroup that the runtime never owned.
type ProcessCleanup struct {
	paths []string
}

func (c *ProcessCleanup) Empty() bool {
	return c == nil || len(c.paths) == 0
}

func (c *ProcessCleanup) Remove(debug bool) error {
	if c == nil {
		return nil
	}
	return removeCgroupPaths(c.paths, debug)
}

// CaptureProcessCleanup records the managed cgroup paths that pid currently
// occupies under name. An empty cleanup token means the process was not in that
// cgroup, which is expected when cgroup setup failed before admission.
func CaptureProcessCleanup(name string, pid int) (*ProcessCleanup, error) {
	if err := validateCgroupName(name); err != nil {
		return nil, err
	}
	if pid <= 0 {
		return nil, fmt.Errorf("invalid cgroup target PID %d", pid)
	}

	data, err := os.ReadFile(filepath.Join("/proc", strconv.Itoa(pid), "cgroup"))
	if err != nil {
		return nil, fmt.Errorf("read process cgroup membership for PID %d: %w", pid, err)
	}
	paths, err := processCleanupPaths(name, string(data), isV2())
	if err != nil {
		return nil, err
	}
	return &ProcessCleanup{paths: paths}, nil
}

func processCleanupPaths(name, membership string, v2 bool) ([]string, error) {
	if err := validateCgroupName(name); err != nil {
		return nil, err
	}

	expected := "/" + name
	seen := make(map[string]struct{})
	paths := make([]string, 0, 3)
	for _, line := range strings.Split(strings.TrimSpace(membership), "\n") {
		if line == "" {
			continue
		}
		parts := strings.SplitN(line, ":", 3)
		if len(parts) != 3 {
			return nil, fmt.Errorf("malformed process cgroup membership %q", line)
		}
		if parts[2] != expected {
			continue
		}

		if v2 {
			if parts[0] != "0" || parts[1] != "" {
				continue
			}
			path := filepath.Join(cgroupV2Root, name)
			if _, ok := seen[path]; !ok {
				seen[path] = struct{}{}
				paths = append(paths, path)
			}
			continue
		}

		for _, controller := range strings.Split(parts[1], ",") {
			switch controller {
			case "memory", "cpu", "pids":
				path := filepath.Join("/sys/fs/cgroup", controller, name)
				if _, ok := seen[path]; !ok {
					seen[path] = struct{}{}
					paths = append(paths, path)
				}
			}
		}
	}
	return paths, nil
}
