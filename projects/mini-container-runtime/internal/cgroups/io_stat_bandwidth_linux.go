//go:build linux

package cgroups

import (
	"bufio"
	"os"
	"path/filepath"
	"strconv"
	"strings"
)

// DeviceIOStat contains parsed I/O metrics for a single block device major:minor.
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

// ReadIOStatSummary parses the io.stat cgroup interface file.
func ReadIOStatSummary(cgroupPath string) (IOStatSummary, error) {
	var summary IOStatSummary
	file, err := os.Open(filepath.Join(cgroupPath, "io.stat"))
	if err != nil {
		if os.IsNotExist(err) {
			return summary, nil
		}
		return summary, err
	}
	defer file.Close()

	scanner := bufio.NewScanner(file)
	for scanner.Scan() {
		fields := strings.Fields(scanner.Text())
		if len(fields) < 2 {
			continue
		}

		devStat := DeviceIOStat{DeviceMajorMinor: fields[0]}
		for _, field := range fields[1:] {
			parts := strings.SplitN(field, "=", 2)
			if len(parts) != 2 {
				continue
			}
			val, _ := strconv.ParseUint(parts[1], 10, 64)
			switch parts[0] {
			case "rbytes":
				devStat.RBytes = val
			case "wbytes":
				devStat.WBytes = val
			case "rios":
				devStat.RIOs = val
			case "wios":
				devStat.WIOs = val
			case "dbytes":
				devStat.DBytes = val
			case "dios":
				devStat.DIOs = val
			}
		}

		summary.TotalRBytes += devStat.RBytes
		summary.TotalWBytes += devStat.WBytes
		summary.TotalRIOs += devStat.RIOs
		summary.TotalWIOs += devStat.WIOs
		summary.Devices = append(summary.Devices, devStat)
	}

	return summary, nil
}
