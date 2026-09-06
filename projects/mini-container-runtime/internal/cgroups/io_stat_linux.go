//go:build linux

package cgroups

import (
	"bufio"
	"os"
	"path/filepath"
	"strconv"
	"strings"
)

// ReadIOStat reads Cgroup v2 io.stat per-device disk I/O metrics.
func ReadIOStat(cgroupPath string) (map[string]uint64, error) {
	statFile := filepath.Join(cgroupPath, "io.stat")
	file, err := os.Open(statFile)
	if err != nil {
		return nil, err
	}
	defer file.Close()

	metrics := make(map[string]uint64)
	scanner := bufio.NewScanner(file)
	for scanner.Scan() {
		fields := strings.Fields(scanner.Text())
		for _, field := range fields[1:] {
			parts := strings.Split(field, "=")
			if len(parts) == 2 {
				val, _ := strconv.ParseUint(parts[1], 10, 64)
				metrics[parts[0]] += val
			}
		}
	}

	return metrics, nil
}
