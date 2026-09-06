//go:build linux

package cgroups

import (
	"bufio"
	"os"
	"path/filepath"
	"strconv"
	"strings"
)

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
	ThrottleRatio float64 // ThrottledUsec / UsageUsec (or nr_throttled / nr_periods)
	BurstRatio    float64 // BurstUsec / UsageUsec
}

// ReadCPUBurstThrottledMetrics reads and calculates throttle & burst statistics from cpu.stat.
func ReadCPUBurstThrottledMetrics(cgroupPath string) (CPUBurstThrottledMetrics, error) {
	var m CPUBurstThrottledMetrics
	file, err := os.Open(filepath.Join(cgroupPath, "cpu.stat"))
	if err != nil {
		if os.IsNotExist(err) {
			return m, nil
		}
		return m, err
	}
	defer file.Close()

	scanner := bufio.NewScanner(file)
	for scanner.Scan() {
		fields := strings.Fields(scanner.Text())
		if len(fields) != 2 {
			continue
		}
		val, _ := strconv.ParseUint(fields[1], 10, 64)
		switch fields[0] {
		case "usage_usec":
			m.UsageUsec = val
		case "user_usec":
			m.UserUsec = val
		case "system_usec":
			m.SystemUsec = val
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

	if m.NrPeriods > 0 {
		m.ThrottleRatio = float64(m.NrThrottled) / float64(m.NrPeriods)
	}
	if m.UsageUsec > 0 {
		m.BurstRatio = float64(m.BurstUsec) / float64(m.UsageUsec)
	}

	return m, nil
}
