//go:build linux

package cgroups

import (
	"bytes"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"strconv"
)

type updateFileOps struct {
	readFile  func(string) ([]byte, error)
	writeFile func(string, []byte, os.FileMode) error
}

type plannedResourceUpdate struct {
	name  string
	value string
}

type savedResourceUpdate struct {
	plannedResourceUpdate
	oldValue []byte
}

func plannedResourceUpdates(cfg UpdateConfig) []plannedResourceUpdate {
	updates := make([]plannedResourceUpdate, 0, 4)
	if cfg.MemoryMax > 0 {
		updates = append(updates, plannedResourceUpdate{
			name:  "memory.max",
			value: strconv.FormatInt(cfg.MemoryMax, 10),
		})
	}
	if cfg.CPUs > 0 {
		quotaUsec := int64(cfg.CPUs * float64(cpuPeriodUsec))
		updates = append(updates, plannedResourceUpdate{
			name:  "cpu.max",
			value: fmt.Sprintf("%d %d", quotaUsec, cpuPeriodUsec),
		})
	}
	if cfg.CPUWeight > 0 {
		updates = append(updates, plannedResourceUpdate{
			name:  "cpu.weight",
			value: strconv.FormatInt(cfg.CPUWeight, 10),
		})
	}
	if cfg.PidsMax > 0 {
		updates = append(updates, plannedResourceUpdate{
			name:  "pids.max",
			value: strconv.FormatInt(cfg.PidsMax, 10),
		})
	}
	return updates
}

func applyResourceUpdateTransaction(
	cgPath string,
	cfg UpdateConfig,
	debug bool,
	ops updateFileOps,
) error {
	if err := validateResourceValues(cfg.MemoryMax, cfg.CPUWeight, cfg.CPUs, cfg.PidsMax); err != nil {
		return err
	}
	if ops.readFile == nil || ops.writeFile == nil {
		return fmt.Errorf("cgroup update file operations are incomplete")
	}

	plan := plannedResourceUpdates(cfg)
	if len(plan) == 0 {
		return nil
	}

	saved := make([]savedResourceUpdate, 0, len(plan))
	for _, update := range plan {
		path := filepath.Join(cgPath, update.name)
		oldValue, err := ops.readFile(path)
		if err != nil {
			return fmt.Errorf("read current %s before update: %w", update.name, err)
		}
		oldValue = bytes.TrimSpace(oldValue)
		if len(oldValue) == 0 {
			return fmt.Errorf("read current %s before update: empty value", update.name)
		}
		saved = append(saved, savedResourceUpdate{
			plannedResourceUpdate: update,
			oldValue:              append([]byte(nil), oldValue...),
		})
	}

	for i, update := range saved {
		path := filepath.Join(cgPath, update.name)
		if err := ops.writeFile(path, []byte(update.value), 0o644); err != nil {
			resultErr := error(fmt.Errorf("update %s: %w", update.name, err))
			for rollbackIndex := i; rollbackIndex >= 0; rollbackIndex-- {
				rollback := saved[rollbackIndex]
				rollbackPath := filepath.Join(cgPath, rollback.name)
				if rollbackErr := ops.writeFile(rollbackPath, rollback.oldValue, 0o644); rollbackErr != nil {
					resultErr = errors.Join(
						resultErr,
						fmt.Errorf("rollback %s after update failure: %w", rollback.name, rollbackErr),
					)
				}
			}
			return resultErr
		}
	}

	if debug {
		for _, update := range saved {
			fmt.Printf("[cgroup] dynamically updated %s = %s\n", update.name, update.value)
		}
	}
	return nil
}
