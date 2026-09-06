//go:build !linux

package cgroups

// ReadLocalSwapFailcntCount is a non-Linux stub.
func ReadLocalSwapFailcntCount(cgroupPath string) (uint64, error) {
	return 0, nil
}
