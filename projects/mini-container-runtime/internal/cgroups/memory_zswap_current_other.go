//go:build !linux

package cgroups

// ReadMemoryZswapCurrent is a non-Linux stub.
func ReadMemoryZswapCurrent(cgroupPath string) (uint64, error) {
	return 0, nil
}
