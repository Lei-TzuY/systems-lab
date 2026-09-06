//go:build !linux

package cgroups

// DeviceIOStat contains parsed I/O metrics for a single block device.
type DeviceIOStat struct {
	DeviceMajorMinor string
	RBytes           uint64
	WBytes           uint64
	RIOs             uint64
	WIOs             uint64
	DBytes           uint64
	DIOs             uint64
}

// IOStatSummary holds total aggregated I/O throughput across all devices.
type IOStatSummary struct {
	TotalRBytes uint64
	TotalWBytes uint64
	TotalRIOs   uint64
	TotalWIOs   uint64
	Devices     []DeviceIOStat
}

// ReadIOStatSummary is a non-Linux stub.
func ReadIOStatSummary(cgroupPath string) (IOStatSummary, error) {
	return IOStatSummary{}, nil
}
