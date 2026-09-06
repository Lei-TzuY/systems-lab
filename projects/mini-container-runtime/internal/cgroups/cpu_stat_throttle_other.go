//go:build !linux

package cgroups

// CPUStatThrottleMetrics holds period-level throttling counters from cpu.stat.
type CPUStatThrottleMetrics struct {
	NrPeriods     uint64
	NrThrottled   uint64
	ThrottledUsec uint64
	NrBursts      uint64
	BurstUsec     uint64
}

// ThrottlePercent returns the percentage of periods that were throttled.
func (m CPUStatThrottleMetrics) ThrottlePercent() float64 {
	if m.NrPeriods == 0 {
		return 0
	}
	return (float64(m.NrThrottled) / float64(m.NrPeriods)) * 100.0
}

// ReadCPUStatThrottleMetrics is a non-Linux stub.
func ReadCPUStatThrottleMetrics(cgroupPath string) (CPUStatThrottleMetrics, error) {
	return CPUStatThrottleMetrics{}, nil
}
