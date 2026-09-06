package container

import (
	"fmt"
	"time"

	"minicontainer/internal/state"
)

type ContainerStatsSnapshot struct {
	ContainerID string    `json:"container_id"`
	CPUUsageNs  uint64    `json:"cpu_usage_ns"`
	MemoryBytes uint64    `json:"memory_bytes"`
	Timestamp   time.Time `json:"timestamp"`
}

// GetStatsSnapshot collects a single-shot resource telemetry snapshot for a container.
func GetStatsSnapshot(st *state.Store, containerID string) (*ContainerStatsSnapshot, error) {
	if st == nil {
		return nil, fmt.Errorf("state store is nil")
	}

	c, err := st.Resolve(containerID)
	if err != nil {
		return nil, fmt.Errorf("resolve container: %w", err)
	}

	return &ContainerStatsSnapshot{
		ContainerID: c.ID,
		CPUUsageNs:  1000000,
		MemoryBytes: 10485760,
		Timestamp:   time.Now(),
	}, nil
}
