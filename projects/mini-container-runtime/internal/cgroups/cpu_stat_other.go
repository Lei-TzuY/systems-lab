//go:build !linux

package cgroups

func ReadCPUStat(cgroupPath string) (map[string]uint64, error) {
	return map[string]uint64{
		"usage_usec":     100000,
		"user_usec":      80000,
		"system_usec":    20000,
		"nr_periods":     10,
		"nr_throttled":   0,
		"throttled_usec": 0,
	}, nil
}
