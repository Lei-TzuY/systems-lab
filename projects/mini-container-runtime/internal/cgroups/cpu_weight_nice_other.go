//go:build !linux

package cgroups

import "fmt"

// ReadCPUWeightNice is a non-Linux stub.
func ReadCPUWeightNice(cgroupPath string) (int, error) {
	return 0, nil
}

// WriteCPUWeightNice is a non-Linux stub.
func WriteCPUWeightNice(cgroupPath string, nice int) error {
	if nice < -20 || nice > 19 {
		return fmt.Errorf("invalid nice value %d (must be -20 to 19)", nice)
	}
	return nil
}
