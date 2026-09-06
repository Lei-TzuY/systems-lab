//go:build linux

package cgroups

import (
	"bufio"
	"os"
	"path/filepath"
	"strconv"
	"strings"
)

// ReadPIDSEventsMaxCount parses the max event counter from pids.events.
// This counter measures the number of times fork(2) or clone(2) failed due to hitting pids.max.
func ReadPIDSEventsMaxCount(cgroupPath string) (uint64, error) {
	eventsFile := filepath.Join(cgroupPath, "pids.events")
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
		if len(fields) == 2 && fields[0] == "max" {
			return strconv.ParseUint(fields[1], 10, 64)
		}
	}

	return 0, nil
}
