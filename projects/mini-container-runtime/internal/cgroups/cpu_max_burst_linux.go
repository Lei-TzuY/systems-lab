//go:build linux

package cgroups

import (
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"strconv"
	"strings"
)

// ReadCPUMaxBurst reads the accumulated burst quota budget in microseconds from cpu.max.burst.
func ReadCPUMaxBurst(cgroupPath string) (uint64, error) {
	if cgroupPath == "" {
		return 0, errors.New("cgroup path is empty")
	}

	burstFile := filepath.Join(cgroupPath, "cpu.max.burst")
	data, err := os.ReadFile(burstFile)
	if err != nil {
		if os.IsNotExist(err) {
			return 0, fmt.Errorf("%w: %s", ErrCPUBurstUnavailable, burstFile)
		}
		return 0, fmt.Errorf("read cpu.max.burst: %w", err)
	}

	val := strings.TrimSpace(string(data))
	if val == "" {
		return 0, fmt.Errorf("parse cpu.max.burst: empty value")
	}

	burst, err := strconv.ParseUint(val, 10, 64)
	if err != nil {
		return 0, fmt.Errorf("parse cpu.max.burst value %q: %w", val, err)
	}
	return burst, nil
}

// WriteCPUMaxBurst sets the CPU max burst quota in microseconds in cpu.max.burst.
func WriteCPUMaxBurst(cgroupPath string, burstUsec uint64) error {
	if cgroupPath == "" {
		return errors.New("cgroup path is empty")
	}

	burstFile := filepath.Join(cgroupPath, "cpu.max.burst")
	if _, err := os.Stat(burstFile); err != nil {
		if os.IsNotExist(err) {
			return fmt.Errorf("%w: %s", ErrCPUBurstUnavailable, burstFile)
		}
		return fmt.Errorf("stat cpu.max.burst: %w", err)
	}

	if err := os.WriteFile(burstFile, []byte(fmt.Sprintf("%d\n", burstUsec)), 0644); err != nil {
		return fmt.Errorf("write cpu.max.burst: %w", err)
	}
	return nil
}
