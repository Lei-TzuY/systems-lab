//go:build !linux

package cgroups

// ReadLocalSwapHighCount is a non-Linux stub.
func ReadLocalSwapHighCount(cgroupPath string) (uint64, error) {
	return 0, nil
}
