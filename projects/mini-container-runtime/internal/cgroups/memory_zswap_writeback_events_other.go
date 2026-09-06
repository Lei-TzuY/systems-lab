//go:build !linux

package cgroups

// ReadMemoryEventsZswapWritebackCount is a non-Linux stub.
func ReadMemoryEventsZswapWritebackCount(cgroupPath string) (uint64, error) {
	return 0, nil
}
