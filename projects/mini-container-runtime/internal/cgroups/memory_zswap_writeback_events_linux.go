//go:build linux

package cgroups

import (
	"bufio"
	"os"
	"path/filepath"
	"strconv"
	"strings"
)

// ReadMemoryEventsZswapWritebackCount parses the zswap_writeback counter from memory.events.
// This counter measures the number of compressed zswap pages evicted to disk swap.
func ReadMemoryEventsZswapWritebackCount(cgroupPath string) (uint64, error) {
	eventsFile := filepath.Join(cgroupPath, "memory.events")
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
		if len(fields) == 2 && fields[0] == "zswap_writeback" {
			return strconv.ParseUint(fields[1], 10, 64)
		}
	}

	return 0, nil
}
