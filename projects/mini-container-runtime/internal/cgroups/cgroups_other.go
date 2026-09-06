//go:build !linux

// internal/cgroups/cgroups_other.go
// Non-Linux build stub for cgroups.

package cgroups

import "fmt"

type Config struct {
	Name      string
	MemoryMax int64
	CPUWeight int64
	CPUs      float64
	PidsMax   int64
}

type Stats struct {
	MemoryUsage    int64
	MemoryLimit    int64
	PidsCurrent    int64
	CPUUsageUsec   uint64
	CPUPressure    *PSIStats
	MemoryPressure *PSIStats
	IOPressure     *PSIStats
}

func Apply(pid int, cfg Config, debug bool) error {
	return fmt.Errorf("cgroups resource limits require Linux")
}

func Remove(name string, debug bool) {}

func ReadStats(name string) (*Stats, error) {
	return nil, fmt.Errorf("cgroups stats require Linux")
}

func Freeze(name string) error {
	return fmt.Errorf("cgroups freeze requires Linux")
}

func Unfreeze(name string) error {
	return fmt.Errorf("cgroups unfreeze requires Linux")
}

func IsFrozen(name string) (bool, error) {
	return false, fmt.Errorf("cgroups freeze requires Linux")
}
