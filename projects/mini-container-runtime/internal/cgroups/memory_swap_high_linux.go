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

// ReadMemorySwapCurrent reads the current swap usage in bytes from memory.swap.current.
func ReadMemorySwapCurrent(cgroupPath string) (uint64, error) {
	if cgroupPath == "" {
		return 0, errors.New("cgroup path is empty")
	}

	target := filepath.Join(cgroupPath, "memory.swap.current")
	data, err := os.ReadFile(target)
	if err != nil {
		if os.IsNotExist(err) {
			return 0, fmt.Errorf("%w: %s", ErrMemorySwapUnavailable, target)
		}
		return 0, fmt.Errorf("read memory.swap.current: %w", err)
	}

	val := strings.TrimSpace(string(data))
	if val == "" {
		return 0, fmt.Errorf("parse memory.swap.current: empty value")
	}
	v, err := strconv.ParseUint(val, 10, 64)
	if err != nil {
		return 0, fmt.Errorf("parse memory.swap.current value %q: %w", val, err)
	}
	return v, nil
}

// ReadMemorySwapHigh reads the swap high watermark from memory.swap.high.
// Returns "max" as 0 with isMax=true.
func ReadMemorySwapHigh(cgroupPath string) (uint64, bool, error) {
	if cgroupPath == "" {
		return 0, false, errors.New("cgroup path is empty")
	}

	target := filepath.Join(cgroupPath, "memory.swap.high")
	data, err := os.ReadFile(target)
	if err != nil {
		if os.IsNotExist(err) {
			return 0, false, fmt.Errorf("%w: %s", ErrMemorySwapUnavailable, target)
		}
		return 0, false, fmt.Errorf("read memory.swap.high: %w", err)
	}

	val := strings.TrimSpace(string(data))
	if val == "max" {
		return 0, true, nil
	}
	if val == "" {
		return 0, false, fmt.Errorf("parse memory.swap.high: empty value")
	}

	v, err := strconv.ParseUint(val, 10, 64)
	if err != nil {
		return 0, false, fmt.Errorf("parse memory.swap.high value %q: %w", val, err)
	}
	return v, false, nil
}

// WriteMemorySwapHigh sets the swap high watermark in bytes, or "max" for unlimited.
func WriteMemorySwapHigh(cgroupPath string, limitBytes int64) error {
	if cgroupPath == "" {
		return errors.New("cgroup path is empty")
	}

	target := filepath.Join(cgroupPath, "memory.swap.high")
	if _, err := os.Stat(target); err != nil {
		if os.IsNotExist(err) {
			return fmt.Errorf("%w: %s", ErrMemorySwapUnavailable, target)
		}
		return fmt.Errorf("stat memory.swap.high: %w", err)
	}

	var content string
	if limitBytes < 0 {
		content = "max\n"
	} else {
		content = fmt.Sprintf("%d\n", limitBytes)
	}
	if err := os.WriteFile(target, []byte(content), 0644); err != nil {
		return fmt.Errorf("write memory.swap.high: %w", err)
	}
	return nil
}
