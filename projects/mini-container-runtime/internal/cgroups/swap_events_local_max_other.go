//go:build !linux

package cgroups

// ReadLocalSwapMaxCount is a non-Linux stub.
func ReadLocalSwapMaxCount(cgroupPath string) (uint64, error) {
	return 0, nil
}
