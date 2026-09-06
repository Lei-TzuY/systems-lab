package stats

import (
	"fmt"
	"time"

	"minicontainer/internal/state"
)

// CollectStatsSampled collects two cgroup snapshots separated by interval and
// derives CPUPercent from the delta in cumulative cpu.stat usage_usec. All
// containers are sampled in one shared interval, so collection latency remains
// roughly interval regardless of container count.
func CollectStatsSampled(st *state.Store, interval time.Duration) ([]ContainerStat, error) {
	if interval <= 0 {
		return nil, fmt.Errorf("sample interval must be positive")
	}

	before, err := CollectStats(st)
	if err != nil {
		return nil, err
	}

	hasCandidate := false
	for _, stat := range before {
		if stat.ProcessLive && stat.Available {
			hasCandidate = true
			break
		}
	}
	if !hasCandidate {
		return before, nil
	}

	t0 := time.Now()
	time.Sleep(interval)
	elapsed := time.Since(t0)

	after, err := CollectStats(st)
	if err != nil {
		return nil, err
	}

	type sampleKey struct {
		id        string
		pid       int
		startTime uint64
	}
	previous := make(map[sampleKey]ContainerStat, len(before))
	for _, stat := range before {
		if stat.ProcessLive && stat.Available {
			previous[sampleKey{id: stat.ContainerID, pid: stat.PID, startTime: stat.PIDStartTime}] = stat
		}
	}

	for i := range after {
		current := &after[i]
		if !current.ProcessLive || !current.Available {
			continue
		}
		prev, ok := previous[sampleKey{id: current.ContainerID, pid: current.PID, startTime: current.PIDStartTime}]
		if !ok {
			continue
		}
		current.CPUPercent = calculateCPUPercent(prev.CPUUsageUsec, current.CPUUsageUsec, elapsed)
	}

	return after, nil
}

func calculateCPUPercent(beforeUsec, afterUsec uint64, elapsed time.Duration) float64 {
	if elapsed <= 0 || afterUsec < beforeUsec {
		return 0
	}
	elapsedUsec := elapsed.Microseconds()
	if elapsedUsec <= 0 {
		return 0
	}
	return float64(afterUsec-beforeUsec) / float64(elapsedUsec) * 100
}
