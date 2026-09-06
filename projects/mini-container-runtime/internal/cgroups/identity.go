package cgroups

import (
	"fmt"
	"strconv"
)

const managedCgroupPrefix = "minicontainer-"

// NameForContainerProcess returns the cgroup name for one exact container
// process generation. The full persisted process identity is PID plus start
// time: PID prevents same-tick restarts from colliding, while start time keeps
// later PID reuse from referring to the same cgroup generation.
func NameForContainerProcess(containerID string, pid int, pidStartTime uint64) (string, error) {
	if containerID == "" {
		return "", fmt.Errorf("container ID must not be empty")
	}
	if pid <= 0 {
		return "", fmt.Errorf("PID must be positive")
	}
	if pidStartTime == 0 {
		return "", fmt.Errorf("PID start time must not be zero")
	}

	name := managedCgroupPrefix + containerID + "-" + strconv.Itoa(pid) + "-" + strconv.FormatUint(pidStartTime, 10)
	if err := validateCgroupName(name); err != nil {
		return "", fmt.Errorf("derive cgroup name: %w", err)
	}
	return name, nil
}
