//go:build linux

package cgroups

import (
	"bufio"
	"os"
	"path/filepath"
	"strconv"
	"strings"
)

// ReadSwapEvents reads Cgroup v2 memory.swap.events counters.
func ReadSwapEvents(cgroupPath string) (map[string]uint64, error) {
	eventsFile := filepath.Join(cgroupPath, "memory.swap.events")
	file, err := os.Open(eventsFile)
	if err != nil {
		return nil, err
	}
	defer file.Close()

	events := make(map[string]uint64)
	scanner := bufio.NewScanner(file)
	for scanner.Scan() {
		fields := strings.Fields(scanner.Text())
		if len(fields) == 2 {
			val, _ := strconv.ParseUint(fields[1], 10, 64)
			events[fields[0]] = val
		}
	}

	return events, nil
}
