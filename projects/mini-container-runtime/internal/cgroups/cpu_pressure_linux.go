//go:build linux

package cgroups

// ReadCPUPressureStallTotal returns the cumulative "some" CPU PSI stall time
// in microseconds. Missing PSI files are treated as unavailable and return 0.
func ReadCPUPressureStallTotal(cgroupPath string) (uint64, error) {
	return readPressureStallTotal(cgroupPath, "cpu")
}
