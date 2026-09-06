//go:build !linux

package cgroups

// CPUBurstThrottledMetrics contains detailed CPU usage, throttling, and bursting stats from cpu.stat.
type CPUBurstThrottledMetrics struct {
	UsageUsec     uint64
	UserUsec      uint64
	SystemUsec    uint64
	NrPeriods     uint64
	NrThrottled   uint64
	ThrottledUsec uint64
	NrBursts      uint64
	BurstUsec     uint64
	ThrottleRatio float64
	BurstRatio    float64
}

// ReadCPUBurstThrottledMetrics is a non-Linux stub.
func ReadCPUBurstThrottledMetrics(cgroupPath string) (CPUBurstThrottledMetrics, error) {
	return CPUBurstThrottledMetrics{}, nil
}
