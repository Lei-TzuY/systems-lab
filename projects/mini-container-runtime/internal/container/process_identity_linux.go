//go:build linux

package container

import (
	"fmt"
	"os"
	"strconv"
	"strings"
)

// ProcessStartTime returns /proc/<pid>/stat field 22: process start time in
// clock ticks since boot. PID alone is insufficient identity because Linux can
// reuse it after a process exits.
func ProcessStartTime(pid int) (uint64, error) {
	if pid <= 0 {
		return 0, fmt.Errorf("invalid PID %d", pid)
	}
	data, err := os.ReadFile(fmt.Sprintf("/proc/%d/stat", pid))
	if err != nil {
		return 0, fmt.Errorf("read process stat for PID %d: %w", pid, err)
	}
	return parseProcessStartTime(string(data))
}

func parseProcessStartTime(stat string) (uint64, error) {
	// comm (field 2) is parenthesized and may itself contain spaces or ')' .
	// The final ')' before field 3 is therefore the only safe split point.
	closeParen := strings.LastIndexByte(stat, ')')
	if closeParen < 0 || closeParen+1 >= len(stat) {
		return 0, fmt.Errorf("malformed /proc stat: missing command terminator")
	}
	fields := strings.Fields(stat[closeParen+1:])
	// fields[0] is field 3 (state), so field 22 is index 19 here.
	if len(fields) <= 19 {
		return 0, fmt.Errorf("malformed /proc stat: missing starttime field")
	}
	start, err := strconv.ParseUint(fields[19], 10, 64)
	if err != nil {
		return 0, fmt.Errorf("parse process starttime %q: %w", fields[19], err)
	}
	return start, nil
}

// ProcessIdentityMatches reports whether PID still names the same live process
// identified by expectedStartTime. Missing/dead processes return false, nil.
func ProcessIdentityMatches(pid int, expectedStartTime uint64) (bool, error) {
	if pid <= 0 || expectedStartTime == 0 {
		return false, nil
	}
	if !IsRunning(pid) {
		return false, nil
	}
	start, err := ProcessStartTime(pid)
	if err != nil {
		if os.IsNotExist(err) {
			return false, nil
		}
		return false, err
	}
	return start == expectedStartTime, nil
}
