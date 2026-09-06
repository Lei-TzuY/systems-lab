//go:build linux

package cgroups

import (
	"bufio"
	"os"
	"path/filepath"
	"strconv"
	"strings"
)

// ReadCPUStat reads Cgroup v2 cpu.stat usage and throttling metrics.
func ReadCPUStat(cgroupPath string) (map[string]uint64, error) {
	statFile := filepath.Join(cgroupPath, "cpu.stat")
	file, err := os.Open(statFile)
	if err != nil {
		return nil, err
	}
	defer file.Close()

	metrics := make(map[string]uint64)
	scanner := bufio.NewScanner(file)
	for scanner.Scan() {
		fields := strings.Fields(scanner.Text())
		if len(fields) == 2 {
			val, _ := strconv.ParseUint(fields[1], 10, 64)
			metrics[fields[0]] = val
		}
	}

	return metrics, nil
}
