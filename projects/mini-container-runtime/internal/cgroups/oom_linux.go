//go:build linux

package cgroups

import (
	"fmt"
	"os"
	"path/filepath"
	"strconv"
	"strings"
)

type OOMEvents struct {
	OOMCount     uint64 `json:"oom_count"`
	OOMKillCount uint64 `json:"oom_kill_count"`
}

// ReadOOMEvents parses Cgroup v2 memory.events to check for OOM kills.
func ReadOOMEvents(cgroupPath string) (*OOMEvents, error) {
	evtFile := filepath.Join(cgroupPath, "memory.events")
	content, err := os.ReadFile(evtFile)
	if err != nil {
		return nil, fmt.Errorf("read memory.events: %w", err)
	}

	res := &OOMEvents{}
	lines := strings.Split(string(content), "\n")
	for _, line := range lines {
		fields := strings.Fields(line)
		if len(fields) == 2 {
			val, _ := strconv.ParseUint(fields[1], 10, 64)
			switch fields[0] {
			case "oom":
				res.OOMCount = val
			case "oom_kill":
				res.OOMKillCount = val
			}
		}
	}

	return res, nil
}
