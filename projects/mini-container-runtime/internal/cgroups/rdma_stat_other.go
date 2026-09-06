//go:build !linux

package cgroups

// DeviceRDMAStat contains RDMA resource allocations for a specific HCA device.
type DeviceRDMAStat struct {
	DeviceName string
	HCAHandle  uint64
	HCAObject  uint64
}

// ReadRDMACurrent is a non-Linux stub.
func ReadRDMACurrent(cgroupPath string) ([]DeviceRDMAStat, error) {
	return nil, nil
}
