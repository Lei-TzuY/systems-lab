//go:build !linux

package cgroups

func ReadSwapFailcntCount(cgroupPath string) (uint64, error) {
	return 0, nil
}
