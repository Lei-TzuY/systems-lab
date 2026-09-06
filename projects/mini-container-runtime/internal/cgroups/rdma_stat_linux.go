//go:build linux

package cgroups

import (
	"bufio"
	"os"
	"path/filepath"
	"strconv"
	"strings"
)

// DeviceRDMAStat contains RDMA resource allocations for a specific HCA device.
type DeviceRDMAStat struct {
	DeviceName string
	HCAHandle  uint64
	HCAObject  uint64
}

// ReadRDMACurrent parses rdma.current in the cgroup directory.
func ReadRDMACurrent(cgroupPath string) ([]DeviceRDMAStat, error) {
	file, err := os.Open(filepath.Join(cgroupPath, "rdma.current"))
	if err != nil {
		if os.IsNotExist(err) {
			return nil, nil
		}
		return nil, err
	}
	defer file.Close()

	var stats []DeviceRDMAStat
	scanner := bufio.NewScanner(file)
	for scanner.Scan() {
		fields := strings.Fields(scanner.Text())
		if len(fields) < 2 {
			continue
		}

		dev := DeviceRDMAStat{DeviceName: fields[0]}
		for _, field := range fields[1:] {
			parts := strings.SplitN(field, "=", 2)
			if len(parts) != 2 {
				continue
			}
			val, _ := strconv.ParseUint(parts[1], 10, 64)
			switch parts[0] {
			case "hca_handle":
				dev.HCAHandle = val
			case "hca_object":
				dev.HCAObject = val
			}
		}
		stats = append(stats, dev)
	}

	return stats, nil
}
