//go:build !linux

package cgroups

import (
	"fmt"
	"strconv"
	"strings"
)

// ReadMemoryZswapMax is a non-Linux stub.
func ReadMemoryZswapMax(cgroupPath string) (string, error) {
	return "max", nil
}

// WriteMemoryZswapMax is a non-Linux stub.
func WriteMemoryZswapMax(cgroupPath string, limit string) error {
	limit = strings.TrimSpace(limit)
	if limit == "" {
		return fmt.Errorf("zswap.max limit cannot be empty")
	}
	if limit != "max" {
		if _, err := strconv.ParseUint(limit, 10, 64); err != nil {
			return fmt.Errorf("invalid zswap.max value %q: %w", limit, err)
		}
	}
	return nil
}
