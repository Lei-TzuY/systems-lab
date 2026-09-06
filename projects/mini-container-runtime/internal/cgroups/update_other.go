//go:build !linux

// internal/cgroups/update_other.go
// Non-Linux build stub for dynamic cgroup updating.

package cgroups

import "fmt"

type UpdateConfig struct {
	MemoryMax int64
	CPUs      float64
	CPUWeight int64
	PidsMax   int64
}

func UpdateLimits(_ string, _ UpdateConfig, _ bool) error {
	return fmt.Errorf("dynamic cgroup resource updating requires Linux")
}
