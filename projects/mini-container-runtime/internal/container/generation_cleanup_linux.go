//go:build linux

package container

import (
	"fmt"

	"minicontainer/internal/cgroups"
)

func cleanupContainerProcessGeneration(containerID string, pid int, pidStartTime uint64) error {
	name, err := cgroups.NameForContainerProcess(containerID, pid, pidStartTime)
	if err != nil {
		return fmt.Errorf("derive cgroup for stopped process generation: %w", err)
	}
	if err := cgroups.RemoveChecked(name, false); err != nil {
		return fmt.Errorf("remove cgroup %s: %w", name, err)
	}
	return nil
}
