//go:build linux

package cgroups

// ReadMemoryPressureStallTotal returns the cumulative "some" memory PSI stall
// time in microseconds. Missing PSI files are treated as unavailable and return 0.
func ReadMemoryPressureStallTotal(cgroupPath string) (uint64, error) {
	return readPressureStallTotal(cgroupPath, "memory")
}
