//go:build linux

package cgroups

import (
	"bufio"
	"os"
	"path/filepath"
	"strconv"
	"strings"
)

// ReadLocalSwapHighCount parses the local high throttle counter from memory.swap.events.local.
// This counter measures swap throttle events encountered directly by the local cgroup.
func ReadLocalSwapHighCount(cgroupPath string) (uint64, error) {
	eventsFile := filepath.Join(cgroupPath, "memory.swap.events.local")
	file, err := os.Open(eventsFile)
	if err != nil {
		if os.IsNotExist(err) {
			return 0, nil
		}
		return 0, err
	}
	defer file.Close()

	scanner := bufio.NewScanner(file)
	for scanner.Scan() {
		fields := strings.Fields(scanner.Text())
		if len(fields) == 2 && fields[0] == "high" {
			return strconv.ParseUint(fields[1], 10, 64)
		}
	}

	return 0, nil
}
