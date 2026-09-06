package metrics

import (
	"os"
	"strings"
	"testing"
	"time"

	"minicontainer/internal/cgroups"
	"minicontainer/internal/container"
	"minicontainer/internal/state"
	runtimestats "minicontainer/internal/stats"
)

func TestPrometheusMetricsDistinguishesVerifiedAndStaleRunningState(t *testing.T) {
	st, err := state.Open(t.TempDir())
	if err != nil {
		t.Fatalf("Open state store error: %v", err)
	}
	start, err := container.ProcessStartTime(os.Getpid())
	if err != nil {
		t.Fatalf("ProcessStartTime: %v", err)
	}

	for _, c := range []*state.Container{
		{
			ID:           "ctr-live",
			PID:          os.Getpid(),
			PIDStartTime: start,
			Hostname:     "live-host",
			Status:       state.StatusRunning,
			Health:       "healthy",
			CreatedAt:    time.Now(),
		},
		{
			ID:           "ctr-stale",
			PID:          os.Getpid(),
			PIDStartTime: start + 1,
			Hostname:     "stale-host",
			Status:       state.StatusRunning,
			CreatedAt:    time.Now(),
		},
	} {
		if err := st.Save(c); err != nil {
			t.Fatalf("Save container error: %v", err)
		}
	}

	out, err := GeneratePrometheusMetrics(st)
	if err != nil {
		t.Fatalf("GeneratePrometheusMetrics error: %v", err)
	}

	checks := []string{
		`minictl_container_status{id="ctr-live"} 1`,
		`minictl_container_state_stale{id="ctr-live"} 0`,
		`minictl_container_status{id="ctr-stale"} 0`,
		`minictl_container_state_stale{id="ctr-stale"} 1`,
		"minictl_images_total 0",
		"# TYPE minictl_container_pressure_stall_seconds_total counter",
	}
	for _, want := range checks {
		if !strings.Contains(out, want) {
			t.Fatalf("metrics missing %q:\n%s", want, out)
		}
	}
}

func TestAppendResourceMetrics(t *testing.T) {
	full := &cgroups.PSIValues{Avg10: 0.25, Avg60: 0.50, Avg300: 0.75, Total: 250000}
	containerStats := []runtimestats.ContainerStat{
		{
			ContainerID:   "ctr-live",
			ProcessLive:   true,
			Available:     true,
			CPUUsageUsec:  1250000,
			MemBytes:      64 * 1024 * 1024,
			MemLimitBytes: 128 * 1024 * 1024,
			PIDs:          7,
			CPUPressure: &cgroups.PSIStats{
				Some: cgroups.PSIValues{Avg10: 1.25, Avg60: 2.50, Avg300: 3.75, Total: 500000},
			},
			MemoryPressure: &cgroups.PSIStats{
				Some: cgroups.PSIValues{Avg10: 4.00, Avg60: 5.00, Avg300: 6.00, Total: 1000000},
				Full: full,
			},
		},
		{
			ContainerID:  "ctr-unavailable",
			ProcessLive:  true,
			Available:    false,
			CPUUsageUsec: 9999999,
			MemBytes:     999,
		},
		{
			ContainerID:  "ctr-stale",
			ProcessLive:  false,
			Available:    true,
			CPUUsageUsec: 9999999,
			MemBytes:     999,
		},
	}

	var sb strings.Builder
	appendResourceMetrics(&sb, containerStats)
	out := sb.String()

	checks := []string{
		`minictl_container_cpu_usage_seconds_total{id="ctr-live"} 1.250000`,
		`minictl_container_memory_usage_bytes{id="ctr-live"} 67108864`,
		`minictl_container_memory_limit_bytes{id="ctr-live"} 134217728`,
		`minictl_container_pids_current{id="ctr-live"} 7`,
		`minictl_container_pressure_avg_percent{id="ctr-live",resource="cpu",scope="some",window="10"} 1.250000`,
		`minictl_container_pressure_stall_seconds_total{id="ctr-live",resource="cpu",scope="some"} 0.500000`,
		`minictl_container_pressure_avg_percent{id="ctr-live",resource="memory",scope="full",window="300"} 0.750000`,
		`minictl_container_pressure_stall_seconds_total{id="ctr-live",resource="memory",scope="full"} 0.250000`,
	}
	for _, want := range checks {
		if !strings.Contains(out, want) {
			t.Fatalf("resource metrics missing %q:\n%s", want, out)
		}
	}
	if strings.Contains(out, "ctr-unavailable") || strings.Contains(out, "ctr-stale") {
		t.Fatalf("unavailable or stale identity must not emit resource samples:\n%s", out)
	}
}
