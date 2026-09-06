//go:build !linux

package cgroups

func ApplyPIDsLimit(cgroupPath string, maxPIDs int64) error {
	return nil
}

func ReadPIDsCurrent(cgroupPath string) (int64, error) {
	return 1, nil
}
