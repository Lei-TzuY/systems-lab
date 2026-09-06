//go:build !linux

package cgroups

import (
	"fmt"
)

// ReadMemoryOOMGroup is a non-Linux stub.
func ReadMemoryOOMGroup(cgroupPath string) (int, error) {
	return 0, nil
}

// WriteMemoryOOMGroup is a non-Linux stub.
func WriteMemoryOOMGroup(cgroupPath string, enabled int) error {
	if enabled != 0 && enabled != 1 {
		return fmt.Errorf("invalid memory.oom.group value %d (must be 0 or 1)", enabled)
	}
	return nil
}
