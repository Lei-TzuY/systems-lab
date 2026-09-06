package stats

import (
	"os"
	"testing"
	"time"

	"minicontainer/internal/container"
	"minicontainer/internal/state"
)

func TestCollectStatsRejectsNilStore(t *testing.T) {
	if _, err := CollectStats(nil); err == nil {
		t.Fatal("expected nil store error")
	}
}

func TestCollectStatsSkipsStoppedContainer(t *testing.T) {
	st, err := state.Open(t.TempDir())
	if err != nil {
		t.Fatalf("Open state store error: %v", err)
	}

	c := &state.Container{
		ID:        "ctr-stats-stopped",
		PID:       99999,
		Status:    state.StatusStopped,
		CreatedAt: time.Now(),
	}
	if err := st.Save(c); err != nil {
		t.Fatalf("save stopped container: %v", err)
	}

	got, err := CollectStats(st)
	if err != nil {
		t.Fatalf("CollectStats error: %v", err)
	}
	if len(got) != 0 {
		t.Fatalf("CollectStats should return 0 stats for stopped container, got %d", len(got))
	}
}

func TestCollectStatsKeepsVerifiedRunningContainerWhenCgroupUnavailable(t *testing.T) {
	st, err := state.Open(t.TempDir())
	if err != nil {
		t.Fatalf("Open state store error: %v", err)
	}
	start, err := container.ProcessStartTime(os.Getpid())
	if err != nil {
		t.Fatalf("ProcessStartTime: %v", err)
	}

	c := &state.Container{
		ID:           "ctr-stats-running",
		PID:          os.Getpid(),
		PIDStartTime: start,
		Status:       state.StatusRunning,
		CreatedAt:    time.Now(),
	}
	if err := st.Save(c); err != nil {
		t.Fatalf("save running container: %v", err)
	}

	got, err := CollectStats(st)
	if err != nil {
		t.Fatalf("CollectStats error: %v", err)
	}
	if len(got) != 1 {
		t.Fatalf("expected 1 running container stat, got %d", len(got))
	}
	if got[0].ContainerID != c.ID || got[0].PID != c.PID || got[0].PIDStartTime != start {
		t.Fatalf("unexpected stat identity: %+v", got[0])
	}
	if !got[0].ProcessLive {
		t.Fatalf("current process identity should be live: %+v", got[0])
	}
	if !got[0].Available && got[0].UnavailableReason != "cgroup_stats_unavailable" {
		t.Fatalf("unexpected unavailable reason: %+v", got[0])
	}
}

func TestCollectStatsExposesStaleRunningIdentityWithoutResources(t *testing.T) {
	st, err := state.Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	start, err := container.ProcessStartTime(os.Getpid())
	if err != nil {
		t.Fatal(err)
	}
	if err := st.Save(&state.Container{
		ID:           "ctr-stale",
		PID:          os.Getpid(),
		PIDStartTime: start + 1,
		Status:       state.StatusRunning,
		CreatedAt:    time.Now(),
	}); err != nil {
		t.Fatal(err)
	}

	got, err := CollectStats(st)
	if err != nil {
		t.Fatal(err)
	}
	if len(got) != 1 {
		t.Fatalf("expected stale running record to remain observable, got %d", len(got))
	}
	if got[0].ProcessLive || got[0].Available {
		t.Fatalf("stale identity must not expose live resources: %+v", got[0])
	}
	if got[0].UnavailableReason != "process_identity_mismatch_or_dead" {
		t.Fatalf("reason=%q", got[0].UnavailableReason)
	}
}

func TestCollectStatsExposesLegacyRunningRecordAsUnverified(t *testing.T) {
	st, err := state.Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	if err := st.Save(&state.Container{
		ID:        "ctr-legacy",
		PID:       os.Getpid(),
		Status:    state.StatusRunning,
		CreatedAt: time.Now(),
	}); err != nil {
		t.Fatal(err)
	}

	got, err := CollectStats(st)
	if err != nil {
		t.Fatal(err)
	}
	if len(got) != 1 || got[0].ProcessLive || got[0].UnavailableReason != "missing_process_identity" {
		t.Fatalf("unexpected legacy record stats: %+v", got)
	}
}
