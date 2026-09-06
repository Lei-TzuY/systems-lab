//go:build !linux

package cgroups

// ReadIOPressureStallTotal is a non-Linux stub.
func ReadIOPressureStallTotal(cgroupPath string) (uint64, error) {
	return 0, nil
}
