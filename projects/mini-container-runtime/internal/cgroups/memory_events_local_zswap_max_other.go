//go:build !linux

package cgroups

// ReadMemoryEventsLocalZswapMaxCount is a non-Linux stub.
func ReadMemoryEventsLocalZswapMaxCount(cgroupPath string) (uint64, error) {
	return 0, nil
}
