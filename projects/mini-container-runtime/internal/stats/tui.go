package stats

import (
	"fmt"

	"minicontainer/internal/cgroups"
	"minicontainer/internal/container"
	"minicontainer/internal/state"
)

type ContainerStat struct {
	ContainerID       string            `json:"container_id"`
	PID               int               `json:"pid"`
	PIDStartTime      uint64            `json:"pid_start_time,omitempty"`
	Status            string            `json:"status"`
	ProcessLive       bool              `json:"process_live"`
	Available         bool              `json:"available"`
	UnavailableReason string            `json:"unavailable_reason,omitempty"`
	CPUPercent        float64           `json:"cpu_percent"`
	CPUUsageUsec      uint64            `json:"cpu_usage_usec"`
	MemBytes          int64             `json:"mem_bytes"`
	MemLimitBytes     int64             `json:"mem_limit_bytes"`
	PIDs              int               `json:"pids"`
	CPUPressure       *cgroups.PSIStats `json:"cpu_pressure,omitempty"`
	MemoryPressure    *cgroups.PSIStats `json:"memory_pressure,omitempty"`
	IOPressure        *cgroups.PSIStats `json:"io_pressure,omitempty"`
}

// CollectStats fetches current cgroup v2 metrics for persisted running
// containers. Persisted state and verified process liveness are kept separate:
// a stale/reused PID remains visible to operators but never receives resource
// samples from an unrelated process.
func CollectStats(st *state.Store) ([]ContainerStat, error) {
	if st == nil {
		return nil, fmt.Errorf("state store is nil")
	}

	all, err := st.List()
	if err != nil {
		return nil, err
	}

	var results []ContainerStat
	for _, c := range all {
		if c.Status != state.StatusRunning {
			continue
		}

		result := ContainerStat{
			ContainerID:  c.ID,
			PID:          c.PID,
			PIDStartTime: c.PIDStartTime,
			Status:       string(c.Status),
		}

		if c.PID <= 0 || c.PIDStartTime == 0 {
			result.UnavailableReason = "missing_process_identity"
			results = append(results, result)
			continue
		}

		live, err := container.ProcessIdentityMatches(c.PID, c.PIDStartTime)
		if err != nil {
			result.UnavailableReason = "process_identity_error"
			results = append(results, result)
			continue
		}
		if !live {
			result.UnavailableReason = "process_identity_mismatch_or_dead"
			results = append(results, result)
			continue
		}
		result.ProcessLive = true

		cgName, err := cgroups.NameForContainerProcess(c.ID, c.PID, c.PIDStartTime)
		if err != nil {
			result.UnavailableReason = "invalid_cgroup_identity"
			results = append(results, result)
			continue
		}
		snapshot, err := cgroups.ReadStats(cgName)
		if err != nil {
			result.UnavailableReason = "cgroup_stats_unavailable"
			results = append(results, result)
			continue
		}

		// Re-check after the cgroup read. The generation-derived cgroup name keeps
		// a concurrent restart from redirecting this read to the replacement
		// process, while this identity check ensures we do not publish stale data
		// after the original process exited during the snapshot.
		stillLive, err := container.ProcessIdentityMatches(c.PID, c.PIDStartTime)
		if err != nil {
			result.ProcessLive = false
			result.UnavailableReason = "process_identity_error_after_snapshot"
			results = append(results, result)
			continue
		}
		if !stillLive {
			result.ProcessLive = false
			result.UnavailableReason = "process_identity_changed_during_snapshot"
			results = append(results, result)
			continue
		}

		result.Available = true
		result.CPUUsageUsec = snapshot.CPUUsageUsec
		result.MemBytes = snapshot.MemoryUsage
		result.MemLimitBytes = snapshot.MemoryLimit
		result.PIDs = int(snapshot.PidsCurrent)
		result.CPUPressure = snapshot.CPUPressure
		result.MemoryPressure = snapshot.MemoryPressure
		result.IOPressure = snapshot.IOPressure
		results = append(results, result)
	}
	return results, nil
}
