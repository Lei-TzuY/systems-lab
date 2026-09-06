//go:build linux

package cgroups

import (
	"bufio"
	"os"
	"path/filepath"
	"strconv"
	"strings"
)

// ReadSwapMaxCount parses max counter from memory.swap.events.
func ReadSwapMaxCount(cgroupPath string) (uint64, error) {
	eventsFile := filepath.Join(cgroupPath, "memory.swap.events")
	file, err := os.Open(eventsFile)
	if err != nil {
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
