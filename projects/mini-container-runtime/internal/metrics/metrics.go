package metrics

import (
	"fmt"
	"strings"

	"minicontainer/internal/cgroups"
	"minicontainer/internal/state"
	runtimestats "minicontainer/internal/stats"
)

// GeneratePrometheusMetrics generates Prometheus text exposition format metrics
// from persisted container state plus identity-verified live cgroup v2 resource snapshots.
func GeneratePrometheusMetrics(st *state.Store) (string, error) {
	if st == nil {
		return "", fmt.Errorf("state store is nil")
	}

	ctrs, err := st.List()
	if err != nil {
		return "", err
	}

	imgs, err := st.ListImages()
	if err != nil {
		return "", err
	}

	resourceStats, err := runtimestats.CollectStats(st)
	if err != nil {
		return "", err
	}
	liveByID := make(map[string]runtimestats.ContainerStat, len(resourceStats))
	for _, stat := range resourceStats {
		liveByID[stat.ContainerID] = stat
	}

	var sb strings.Builder

	sb.WriteString("# HELP minictl_container_info Container persisted metadata\n")
	sb.WriteString("# TYPE minictl_container_info gauge\n")
	for _, c := range ctrs {
		sb.WriteString(fmt.Sprintf("minictl_container_info{id=%q,hostname=%q,status=%q,health=%q} 1\n",
			c.ID, c.Hostname, c.Status, c.Health))
	}

	sb.WriteString("\n# HELP minictl_container_status Verified live container state (1 only when persisted running state matches the current process identity)\n")
	sb.WriteString("# TYPE minictl_container_status gauge\n")
	for _, c := range ctrs {
		val := 0
		if c.Status == state.StatusRunning {
			if stat, ok := liveByID[c.ID]; ok && stat.ProcessLive {
				val = 1
			}
		}
		sb.WriteString(fmt.Sprintf("minictl_container_status{id=%q} %d\n", c.ID, val))
	}

	sb.WriteString("\n# HELP minictl_container_state_stale Persisted running state without a matching live process identity\n")
	sb.WriteString("# TYPE minictl_container_state_stale gauge\n")
	for _, c := range ctrs {
		val := 0
		if c.Status == state.StatusRunning {
			stat, ok := liveByID[c.ID]
			if !ok || !stat.ProcessLive {
				val = 1
			}
		}
		sb.WriteString(fmt.Sprintf("minictl_container_state_stale{id=%q} %d\n", c.ID, val))
	}

	sb.WriteString("\n# HELP minictl_container_exit_code Exit code of the container process\n")
	sb.WriteString("# TYPE minictl_container_exit_code gauge\n")
	for _, c := range ctrs {
		sb.WriteString(fmt.Sprintf("minictl_container_exit_code{id=%q} %d\n", c.ID, c.ExitCode))
	}

	appendResourceMetrics(&sb, resourceStats)

	sb.WriteString("\n# HELP minictl_images_total Total registered container rootfs images\n")
	sb.WriteString("# TYPE minictl_images_total gauge\n")
	sb.WriteString(fmt.Sprintf("minictl_images_total %d\n", len(imgs)))

	return sb.String(), nil
}

func appendResourceMetrics(sb *strings.Builder, containerStats []runtimestats.ContainerStat) {
	sb.WriteString("\n# HELP minictl_container_cpu_usage_seconds_total Cumulative container CPU time from cgroup v2\n")
	sb.WriteString("# TYPE minictl_container_cpu_usage_seconds_total counter\n")
	for _, stat := range containerStats {
		if !stat.ProcessLive || !stat.Available {
			continue
		}
		fmt.Fprintf(sb, "minictl_container_cpu_usage_seconds_total{id=%q} %.6f\n",
			stat.ContainerID, float64(stat.CPUUsageUsec)/1_000_000)
	}

	sb.WriteString("\n# HELP minictl_container_memory_usage_bytes Current container memory usage from cgroup v2\n")
	sb.WriteString("# TYPE minictl_container_memory_usage_bytes gauge\n")
	for _, stat := range containerStats {
		if stat.ProcessLive && stat.Available {
			fmt.Fprintf(sb, "minictl_container_memory_usage_bytes{id=%q} %d\n", stat.ContainerID, stat.MemBytes)
		}
	}

	sb.WriteString("\n# HELP minictl_container_memory_limit_bytes Container memory limit in bytes; 0 means unlimited\n")
	sb.WriteString("# TYPE minictl_container_memory_limit_bytes gauge\n")
	for _, stat := range containerStats {
		if stat.ProcessLive && stat.Available {
			fmt.Fprintf(sb, "minictl_container_memory_limit_bytes{id=%q} %d\n", stat.ContainerID, stat.MemLimitBytes)
		}
	}

	sb.WriteString("\n# HELP minictl_container_pids_current Current number of processes/threads in the container cgroup\n")
	sb.WriteString("# TYPE minictl_container_pids_current gauge\n")
	for _, stat := range containerStats {
		if stat.ProcessLive && stat.Available {
			fmt.Fprintf(sb, "minictl_container_pids_current{id=%q} %d\n", stat.ContainerID, stat.PIDs)
		}
	}

	sb.WriteString("\n# HELP minictl_container_pressure_avg_percent PSI stalled-time percentage over the selected averaging window\n")
	sb.WriteString("# TYPE minictl_container_pressure_avg_percent gauge\n")
	sb.WriteString("# HELP minictl_container_pressure_stall_seconds_total Cumulative PSI stall time in seconds\n")
	sb.WriteString("# TYPE minictl_container_pressure_stall_seconds_total counter\n")
	for _, stat := range containerStats {
		if !stat.ProcessLive || !stat.Available {
			continue
		}
		appendPSIMetrics(sb, stat.ContainerID, "cpu", stat.CPUPressure)
		appendPSIMetrics(sb, stat.ContainerID, "memory", stat.MemoryPressure)
		appendPSIMetrics(sb, stat.ContainerID, "io", stat.IOPressure)
	}
}

func appendPSIMetrics(sb *strings.Builder, containerID, resource string, psi *cgroups.PSIStats) {
	if psi == nil {
		return
	}
	appendPSIScopeMetrics(sb, containerID, resource, "some", psi.Some)
	if psi.Full != nil {
		appendPSIScopeMetrics(sb, containerID, resource, "full", *psi.Full)
	}
}

func appendPSIScopeMetrics(sb *strings.Builder, containerID, resource, scope string, values cgroups.PSIValues) {
	for _, sample := range []struct {
		window string
		value  float64
	}{
		{window: "10", value: values.Avg10},
		{window: "60", value: values.Avg60},
		{window: "300", value: values.Avg300},
	} {
		fmt.Fprintf(sb, "minictl_container_pressure_avg_percent{id=%q,resource=%q,scope=%q,window=%q} %.6f\n",
			containerID, resource, scope, sample.window, sample.value)
	}
	fmt.Fprintf(sb, "minictl_container_pressure_stall_seconds_total{id=%q,resource=%q,scope=%q} %.6f\n",
		containerID, resource, scope, float64(values.Total)/1_000_000)
}
