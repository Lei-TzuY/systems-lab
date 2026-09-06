//go:build !linux

package cgroups

// ReadCPUPressureStallTotal is a non-Linux stub.
func ReadCPUPressureStallTotal(cgroupPath string) (uint64, error) {
	return 0, nil
}
