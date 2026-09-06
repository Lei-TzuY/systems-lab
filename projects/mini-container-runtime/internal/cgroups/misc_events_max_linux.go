//go:build linux

package cgroups

import (
	"bufio"
	"os"
	"path/filepath"
	"strconv"
	"strings"
)

// MiscEventMax represents a max capacity limit failure event for a misc hardware resource.
type MiscEventMax struct {
	ResourceName string
	MaxFails     uint64
}

// ReadMiscEventsMax parses misc.events in the cgroup directory.
func ReadMiscEventsMax(cgroupPath string) ([]MiscEventMax, error) {
	file, err := os.Open(filepath.Join(cgroupPath, "misc.events"))
	if err != nil {
		if os.IsNotExist(err) {
			return nil, nil
		}
		return nil, err
	}
	defer file.Close()

	var events []MiscEventMax
	scanner := bufio.NewScanner(file)
	for scanner.Scan() {
		fields := strings.Fields(scanner.Text())
		// Expected format: sev max 10
		if len(fields) == 3 && fields[1] == "max" {
			val, _ := strconv.ParseUint(fields[2], 10, 64)
			events = append(events, MiscEventMax{
				ResourceName: fields[0],
				MaxFails:     val,
			})
		}
	}

	return events, nil
}
