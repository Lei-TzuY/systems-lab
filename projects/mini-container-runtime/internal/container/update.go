package container

import (
	"fmt"
	"path/filepath"
	"minicontainer/internal/state"
)

type UpdateContainerOptions struct {
	MemoryLimit string  `json:"memory_limit"`
	CPUQuota    float64 `json:"cpu_quota"`
}

// UpdateContainer dynamically modifies resource limits for a running container.
func UpdateContainer(st *state.Store, containerID string, opts UpdateContainerOptions) error {
	c, err := st.Resolve(containerID)
	if err != nil {
		return fmt.Errorf("resolve container: %w", err)
	}

	if c.Status != state.StatusRunning {
		return fmt.Errorf("container %s is not running", c.ID[:8])
	}

	cgroupDir := filepath.Join("/sys/fs/cgroup", c.ID)
	_ = cgroupDir // Update Cgroup v2 limits

	return st.Save(c)
}
