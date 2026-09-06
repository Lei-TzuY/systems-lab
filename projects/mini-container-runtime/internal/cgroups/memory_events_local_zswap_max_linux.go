//go:build linux

package cgroups

import (
	"bufio"
	"os"
	"path/filepath"
	"strconv"
	"strings"
)

// ReadMemoryEventsLocalZswapMaxCount parses zswap_max from memory.events.local.
// This measures local (non-hierarchical) zswap pool allocations rejected directly within this cgroup.
func ReadMemoryEventsLocalZswapMaxCount(cgroupPath string) (uint64, error) {
	eventsFile := filepath.Join(cgroupPath, "memory.events.local")
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
		if len(fields) == 2 && fields[0] == "zswap_max" {
			return strconv.ParseUint(fields[1], 10, 64)
		}
	}

	return 0, nil
}
