//go:build linux

package cgroups

// ReadIOPressureStallTotal returns the cumulative "some" I/O PSI stall time
// in microseconds. Missing PSI files are treated as unavailable and return 0.
func ReadIOPressureStallTotal(cgroupPath string) (uint64, error) {
	return readPressureStallTotal(cgroupPath, "io")
}
