//go:build linux

package dns

import (
	"fmt"
	"os"
	"strconv"
	"strings"
)

type registrarIdentity struct {
	PID       int
	StartTime uint64
}

func currentRegistrarIdentity() (registrarIdentity, error) {
	pid := os.Getpid()
	start, err := readProcessStartTime(pid)
	if err != nil {
		return registrarIdentity{}, fmt.Errorf("capture DNS registrar process identity %d: %w", pid, err)
	}
	return registrarIdentity{PID: pid, StartTime: start}, nil
}

func registrarGenerationAlive(pid int, startTime uint64) (bool, error) {
	if pid <= 0 || startTime == 0 {
		return false, fmt.Errorf("invalid DNS registrar process identity %d/%d", pid, startTime)
	}
	current, processState, err := readProcessStat(pid)
	if err != nil {
		if os.IsNotExist(err) {
			return false, nil
		}
		return false, err
	}
	// A zombie still has a /proc entry and the same start time, but it has
	// already exited and cannot own live DNS discovery state. Treat it exactly
	// like an absent generation rather than waiting for some other process to
	// reap it.
	return current == startTime && processState != "Z", nil
}

// readProcessStartTime reads Linux /proc/<pid>/stat field 22. The command name
// in field 2 may contain spaces and ')' characters, so split after the final
// ") " boundary before indexing fields 3..N.
func readProcessStartTime(pid int) (uint64, error) {
	start, _, err := readProcessStat(pid)
	return start, err
}

func readProcessStat(pid int) (uint64, string, error) {
	data, err := os.ReadFile(fmt.Sprintf("/proc/%d/stat", pid))
	if err != nil {
		return 0, "", err
	}
	text := string(data)
	boundary := strings.LastIndex(text, ") ")
	if boundary < 0 {
		return 0, "", fmt.Errorf("malformed /proc/%d/stat: missing comm boundary", pid)
	}
	fields := strings.Fields(text[boundary+2:])
	// fields[0] is stat field 3 (state); field 22 is therefore index 19.
	if len(fields) <= 19 {
		return 0, "", fmt.Errorf("malformed /proc/%d/stat: only %d fields after comm", pid, len(fields))
	}
	if len(fields[0]) != 1 {
		return 0, "", fmt.Errorf("malformed /proc/%d/stat process state %q", pid, fields[0])
	}
	start, err := strconv.ParseUint(fields[19], 10, 64)
	if err != nil || start == 0 {
		if err == nil {
			err = fmt.Errorf("zero start time")
		}
		return 0, "", fmt.Errorf("parse /proc/%d/stat start time: %w", pid, err)
	}
	return start, fields[0], nil
}
