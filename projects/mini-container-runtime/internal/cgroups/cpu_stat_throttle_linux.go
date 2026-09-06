//go:build linux

package cgroups

import (
	"bufio"
	"os"
	"path/filepath"
	"strconv"
	"strings"
)

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

// ReadCPUStatThrottleMetrics reads cpu.stat and extracts throttle/burst counters.
func ReadCPUStatThrottleMetrics(cgroupPath string) (CPUStatThrottleMetrics, error) {
	file, err := os.Open(filepath.Join(cgroupPath, "cpu.stat"))
	if err != nil {
		if os.IsNotExist(err) {
			return CPUStatThrottleMetrics{}, nil
		}
		return CPUStatThrottleMetrics{}, err
	}
	defer file.Close()

	var m CPUStatThrottleMetrics
	scanner := bufio.NewScanner(file)
	for scanner.Scan() {
		fields := strings.Fields(scanner.Text())
		if len(fields) != 2 {
			continue
		}
		val, _ := strconv.ParseUint(fields[1], 10, 64)
		switch fields[0] {
		case "nr_periods":
			m.NrPeriods = val
		case "nr_throttled":
			m.NrThrottled = val
		case "throttled_usec":
			m.ThrottledUsec = val
		case "nr_bursts":
			m.NrBursts = val
		case "burst_usec":
			m.BurstUsec = val
		}
	}

	return m, nil
}
