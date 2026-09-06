//go:build !linux

package container

import (
	"fmt"

	"minicontainer/internal/cgroups"
)

func cleanupContainerProcessGeneration(containerID string, pid int, pidStartTime uint64) error {
	if _, err := cgroups.NameForContainerProcess(containerID, pid, pidStartTime); err != nil {
		return fmt.Errorf("derive cgroup for stopped process generation: %w", err)
	}
	return fmt.Errorf("cgroup generation cleanup requires Linux")
}
