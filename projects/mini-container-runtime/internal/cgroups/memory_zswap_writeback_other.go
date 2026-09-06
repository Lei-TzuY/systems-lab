//go:build !linux

package cgroups

import (
	"fmt"
)

// ReadMemoryZswapWriteback is a non-Linux stub.
func ReadMemoryZswapWriteback(cgroupPath string) (int, error) {
	return 1, nil
}

// WriteMemoryZswapWriteback is a non-Linux stub.
func WriteMemoryZswapWriteback(cgroupPath string, enabled int) error {
	if enabled != 0 && enabled != 1 {
		return fmt.Errorf("invalid memory.zswap.writeback value %d (must be 0 or 1)", enabled)
	}
	return nil
}
