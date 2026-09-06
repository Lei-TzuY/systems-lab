//go:build linux

package cgroups

import (
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"strconv"
)

type v1Write struct {
	file     string
	value    string
	optional bool
}

type v1ControllerPlan struct {
	controller string
	writes     []v1Write
}

func buildV1Plans(cfg Config) []v1ControllerPlan {
	plans := make([]v1ControllerPlan, 0, 3)

	if cfg.MemoryMax > 0 {
		limit := strconv.FormatInt(cfg.MemoryMax, 10)
		plans = append(plans, v1ControllerPlan{
			controller: "memory",
			writes: []v1Write{
				{file: "memory.limit_in_bytes", value: limit},
				{file: "memory.memsw.limit_in_bytes", value: limit, optional: true},
			},
		})
	}

	if cfg.CPUWeight > 0 || cfg.CPUs > 0 {
		cpuPlan := v1ControllerPlan{controller: "cpu"}
		if cfg.CPUWeight > 0 {
			cpuPlan.writes = append(cpuPlan.writes, v1Write{
				file:  "cpu.shares",
				value: strconv.FormatInt(cfg.CPUWeight, 10),
			})
		}
		if cfg.CPUs > 0 {
			const periodUs int64 = 100000
			quotaUs := int64(cfg.CPUs * float64(periodUs))
			cpuPlan.writes = append(cpuPlan.writes,
				v1Write{file: "cpu.cfs_period_us", value: strconv.FormatInt(periodUs, 10)},
				v1Write{file: "cpu.cfs_quota_us", value: strconv.FormatInt(quotaUs, 10)},
			)
		}
		plans = append(plans, cpuPlan)
	}

	if cfg.PidsMax > 0 {
		plans = append(plans, v1ControllerPlan{
			controller: "pids",
			writes: []v1Write{
				{file: "pids.max", value: strconv.FormatInt(cfg.PidsMax, 10)},
			},
		})
	}

	return plans
}

func applyV1At(root string, pid int, cfg Config, debug bool) error {
	plans := buildV1Plans(cfg)
	if len(plans) == 0 {
		return nil
	}

	created := make([]string, 0, len(plans))
	for _, plan := range plans {
		cgPath := filepath.Join(root, plan.controller, cfg.Name)
		if err := os.Mkdir(cgPath, 0o755); err != nil {
			cleanupV1Paths(created, debug)
			if errors.Is(err, os.ErrExist) {
				return fmt.Errorf("cgroup v1 %s already exists; refusing stale reuse", cgPath)
			}
			return fmt.Errorf("mkdir cgroup v1 %s: %w", cgPath, err)
		}
		created = append(created, cgPath)
	}

	if err := applyV1Prepared(root, cfg.Name, pid, plans, debug); err != nil {
		cleanupV1Paths(created, debug)
		return err
	}
	return nil
}

// applyV1Prepared assumes controller cgroup directories already exist. It is
// separated from directory creation so the configuration/admission ordering can
// be tested on an unprivileged fake hierarchy.
func applyV1Prepared(root, name string, pid int, plans []v1ControllerPlan, debug bool) error {
	for _, plan := range plans {
		cgPath := filepath.Join(root, plan.controller, name)
		for _, write := range plan.writes {
			path := filepath.Join(cgPath, write.file)
			if write.optional {
				if _, err := os.Stat(path); errors.Is(err, os.ErrNotExist) {
					continue
				} else if err != nil {
					return fmt.Errorf("inspect cgroup v1 %s: %w", path, err)
				}
			}
			if err := os.WriteFile(path, []byte(write.value), 0o644); err != nil {
				return fmt.Errorf("write cgroup v1 %s: %w", path, err)
			}
			if debug {
				fmt.Printf("[cgroup v1] %s/%s = %s\n", plan.controller, write.file, write.value)
			}
		}
	}

	pidStr := strconv.Itoa(pid)
	attached := make([]string, 0, len(plans))
	for _, plan := range plans {
		tasks := filepath.Join(root, plan.controller, name, "tasks")
		if err := os.WriteFile(tasks, []byte(pidStr), 0o644); err != nil {
			attachErr := fmt.Errorf("attach PID %d to cgroup v1 %s: %w", pid, tasks, err)
			return rollbackV1Attachments(root, pidStr, attached, attachErr, debug)
		}
		attached = append(attached, plan.controller)
		if debug {
			fmt.Printf("[cgroup v1] %s/tasks = %s\n", plan.controller, pidStr)
		}
	}
	return nil
}

func rollbackV1Attachments(root, pid string, controllers []string, cause error, debug bool) error {
	errs := []error{cause}
	for i := len(controllers) - 1; i >= 0; i-- {
		controller := controllers[i]
		parentTasks := filepath.Join(root, controller, "tasks")
		if err := os.WriteFile(parentTasks, []byte(pid), 0o644); err != nil {
			errs = append(errs, fmt.Errorf("rollback PID %s to cgroup v1 %s: %w", pid, parentTasks, err))
			continue
		}
		if debug {
			fmt.Printf("[cgroup v1] rollback %s/tasks = %s\n", controller, pid)
		}
	}
	return errors.Join(errs...)
}

func cleanupV1Paths(paths []string, debug bool) {
	for i := len(paths) - 1; i >= 0; i-- {
		removePath(paths[i], debug)
	}
}
