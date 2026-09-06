//go:build !linux

package cgroups

// ReadMemoryPressureStallTotal is a non-Linux stub.
func ReadMemoryPressureStallTotal(cgroupPath string) (uint64, error) {
	return 0, nil
}
