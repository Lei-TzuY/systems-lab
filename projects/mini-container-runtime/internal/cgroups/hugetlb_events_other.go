//go:build !linux

package cgroups

// ReadHugeTLBEventsMaxCount is a non-Linux stub.
func ReadHugeTLBEventsMaxCount(cgroupPath, pageSize string) (uint64, error) {
	return 0, nil
}

// ReadHugeTLBCurrentBytes is a non-Linux stub.
func ReadHugeTLBCurrentBytes(cgroupPath, pageSize string) (uint64, error) {
	return 0, nil
}
