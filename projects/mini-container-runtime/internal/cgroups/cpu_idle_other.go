//go:build !linux

package cgroups

import (
	"fmt"
)

// ReadCPUIdle is a non-Linux stub.
func ReadCPUIdle(cgroupPath string) (int, error) {
	return 0, nil
}

// WriteCPUIdle is a non-Linux stub.
func WriteCPUIdle(cgroupPath string, idle int) error {
	if idle != 0 && idle != 1 {
		return fmt.Errorf("invalid cpu.idle value %d (must be 0 or 1)", idle)
	}
	return nil
}
