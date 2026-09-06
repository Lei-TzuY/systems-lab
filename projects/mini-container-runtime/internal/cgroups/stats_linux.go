//go:build linux

// internal/cgroups/stats_linux.go
//
// Resource Metrics & Monitoring
// ──────────────────────────────
// Container stats monitoring reads live usage figures directly from Linux
// cgroup v2 pseudo-files:
//
//   • Memory Usage : /sys/fs/cgroup/<name>/memory.current
//   • Memory Limit : /sys/fs/cgroup/<name>/memory.max ("max" = no limit)
//   • Process Count: /sys/fs/cgroup/<name>/pids.current
//   • CPU Usage    : /sys/fs/cgroup/<name>/cpu.stat (usage_usec field)
//   • CPU PSI      : /sys/fs/cgroup/<name>/cpu.pressure
//   • Memory PSI   : /sys/fs/cgroup/<name>/memory.pressure
//   • I/O PSI      : /sys/fs/cgroup/<name>/io.pressure
//
// This is how `docker stats`-style tooling can obtain per-container resource
// usage while PSI adds direct visibility into resource contention and stalls.

package cgroups

import (
	"bufio"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"strconv"
	"strings"
)

// Stats holds a snapshot of container resource metrics.
type Stats struct {
	MemoryUsage    int64  // bytes currently used
	MemoryLimit    int64  // max allowed bytes (0 = unlimited / host limit)
	PidsCurrent    int64  // number of active processes/threads
	CPUUsageUsec   uint64 // total CPU time consumed in microseconds
	CPUPressure    *PSIStats
	MemoryPressure *PSIStats
	IOPressure     *PSIStats
}

// ReadStats reads live cgroup metrics for the given cgroup name (e.g., "minicontainer-1234").
func ReadStats(name string) (*Stats, error) {
	if err := validateCgroupName(name); err != nil {
		return nil, err
	}
	return readStatsAtPath(filepath.Join(cgroupV2Root, name))
}

func readStatsAtPath(cgPath string) (*Stats, error) {
	info, err := os.Stat(cgPath)
	if err != nil {
		if errors.Is(err, os.ErrNotExist) {
			return nil, fmt.Errorf("cgroup %s does not exist: %w", cgPath, err)
		}
		return nil, fmt.Errorf("stat cgroup %s: %w", cgPath, err)
	}
	if !info.IsDir() {
		return nil, fmt.Errorf("cgroup path %s is not a directory", cgPath)
	}

	stats := &Stats{}

	if value, present, err := readOptionalInt64(filepath.Join(cgPath, "memory.current"), "memory.current"); err != nil {
		return nil, err
	} else if present {
		if value < 0 {
			return nil, fmt.Errorf("memory.current must not be negative: %d", value)
		}
		stats.MemoryUsage = value
	}

	if value, present, err := readOptionalMemoryMax(filepath.Join(cgPath, "memory.max")); err != nil {
		return nil, err
	} else if present {
		stats.MemoryLimit = value
	}

	if value, present, err := readOptionalInt64(filepath.Join(cgPath, "pids.current"), "pids.current"); err != nil {
		return nil, err
	} else if present {
		if value < 0 {
			return nil, fmt.Errorf("pids.current must not be negative: %d", value)
		}
		stats.PidsCurrent = value
	}

	if value, present, err := readOptionalCPUUsage(filepath.Join(cgPath, "cpu.stat")); err != nil {
		return nil, err
	} else if present {
		stats.CPUUsageUsec = value
	}

	if psi, err := readOptionalPSI(cgPath, "cpu"); err != nil {
		return nil, err
	} else {
		stats.CPUPressure = psi
	}
	if psi, err := readOptionalPSI(cgPath, "memory"); err != nil {
		return nil, err
	} else {
		stats.MemoryPressure = psi
	}
	if psi, err := readOptionalPSI(cgPath, "io"); err != nil {
		return nil, err
	} else {
		stats.IOPressure = psi
	}

	return stats, nil
}

func readOptionalInt64(path, field string) (int64, bool, error) {
	data, err := os.ReadFile(path)
	if err != nil {
		if errors.Is(err, os.ErrNotExist) {
			return 0, false, nil
		}
		return 0, false, fmt.Errorf("read %s: %w", field, err)
	}

	raw := strings.TrimSpace(string(data))
	value, err := strconv.ParseInt(raw, 10, 64)
	if err != nil {
		return 0, true, fmt.Errorf("parse %s value %q: %w", field, raw, err)
	}
	return value, true, nil
}

func readOptionalMemoryMax(path string) (int64, bool, error) {
	data, err := os.ReadFile(path)
	if err != nil {
		if errors.Is(err, os.ErrNotExist) {
			return 0, false, nil
		}
		return 0, false, fmt.Errorf("read memory.max: %w", err)
	}

	raw := strings.TrimSpace(string(data))
	if raw == "max" {
		return 0, true, nil
	}
	value, err := strconv.ParseInt(raw, 10, 64)
	if err != nil {
		return 0, true, fmt.Errorf("parse memory.max value %q: %w", raw, err)
	}
	if value < 0 {
		return 0, true, fmt.Errorf("memory.max must not be negative: %d", value)
	}
	return value, true, nil
}

func readOptionalCPUUsage(path string) (uint64, bool, error) {
	f, err := os.Open(path)
	if err != nil {
		if errors.Is(err, os.ErrNotExist) {
			return 0, false, nil
		}
		return 0, false, fmt.Errorf("open cpu.stat: %w", err)
	}
	defer f.Close()

	scanner := bufio.NewScanner(f)
	for scanner.Scan() {
		fields := strings.Fields(scanner.Text())
		if len(fields) == 0 || fields[0] != "usage_usec" {
			continue
		}
		if len(fields) != 2 {
			return 0, true, fmt.Errorf("malformed cpu.stat usage_usec line %q", scanner.Text())
		}
		value, err := strconv.ParseUint(fields[1], 10, 64)
		if err != nil {
			return 0, true, fmt.Errorf("parse cpu.stat usage_usec value %q: %w", fields[1], err)
		}
		return value, true, nil
	}
	if err := scanner.Err(); err != nil {
		return 0, true, fmt.Errorf("scan cpu.stat: %w", err)
	}
	return 0, true, fmt.Errorf("cpu.stat missing usage_usec")
}

func readOptionalPSI(cgPath, resource string) (*PSIStats, error) {
	psi, err := ReadPSIStats(cgPath, resource)
	if err != nil {
		if errors.Is(err, os.ErrNotExist) {
			return nil, nil
		}
		return nil, err
	}
	return psi, nil
}
