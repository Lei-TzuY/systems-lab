//go:build !linux

package cgroups

// ReadPIDSEventsMaxCount is a non-Linux stub.
func ReadPIDSEventsMaxCount(cgroupPath string) (uint64, error) {
	return 0, nil
}
