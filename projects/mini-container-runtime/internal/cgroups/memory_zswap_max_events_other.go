//go:build !linux

package cgroups

// ReadMemoryEventsZswapMaxCount is a non-Linux stub.
func ReadMemoryEventsZswapMaxCount(cgroupPath string) (uint64, error) {
	return 0, nil
}
