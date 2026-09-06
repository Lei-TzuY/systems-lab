//go:build !linux

package cgroups

// ReadMemoryEventsLocalZswapWritebackCount is a non-Linux stub.
func ReadMemoryEventsLocalZswapWritebackCount(cgroupPath string) (uint64, error) {
	return 0, nil
}
