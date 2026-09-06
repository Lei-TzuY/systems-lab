//go:build !linux

package cgroups

func ReadMemoryEvents(cgroupPath string) (map[string]uint64, error) {
	return map[string]uint64{
		"low":      0,
		"high":     0,
		"max":      0,
		"oom":      0,
		"oom_kill": 0,
	}, nil
}
