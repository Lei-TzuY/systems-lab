//go:build !linux

package cgroups

func ReadSwapEvents(cgroupPath string) (map[string]uint64, error) {
	return map[string]uint64{
		"high": 0,
		"max":  0,
		"fail": 0,
	}, nil
}
