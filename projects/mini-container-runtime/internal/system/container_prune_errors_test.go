package system

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"minicontainer/internal/state"
)

func openContainerPruneTestStore(t *testing.T) (*state.Store, string) {
	t.Helper()
	base := t.TempDir()
	home := filepath.Join(base, "home")
	if err := os.MkdirAll(home, 0o700); err != nil {
		t.Fatal(err)
	}
	t.Setenv("HOME", home)
	st, err := state.Open(filepath.Join(base, "store"))
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = st.Close() })
	return st, base
}

func saveStoppedPruneContainer(t *testing.T, st *state.Store, id string, createdAt time.Time, pendingCgroup bool) {
	t.Helper()
	const pid = 6161
	const start = 101
	c := &state.Container{
		ID:           id,
		Status:       state.StatusRunning,
		PID:          pid,
		PIDStartTime: start,
		CreatedAt:    createdAt,
	}
	if err := st.Save(c); err != nil {
		t.Fatal(err)
	}
	if pendingCgroup {
		name := "minicontainer-" + id + "-6161-101"
		if err := st.MarkCgroupOwnedIfIdentity(id, pid, start, name); err != nil {
			t.Fatalf("mark cgroup ownership: %v", err)
		}
	}
	if changed, err := st.MarkStoppedIfIdentity(id, pid, start, 0, time.Now()); err != nil || !changed {
		t.Fatalf("mark stopped %s: changed=%v err=%v", id, changed, err)
	}
}

func TestDeletePrunableContainerTreatsRestartAsSkip(t *testing.T) {
	st, _ := openContainerPruneTestStore(t)
	c := &state.Container{
		ID:           "running-prune-skip",
		Status:       state.StatusRunning,
		PID:          7001,
		PIDStartTime: 55,
		CreatedAt:    time.Now().Add(-time.Hour),
	}
	if err := st.Save(c); err != nil {
		t.Fatal(err)
	}

	deleted, err := deletePrunableContainer(st, c.ID)
	if err != nil {
		t.Fatalf("running prune candidate returned error: %v", err)
	}
	if deleted {
		t.Fatal("running generation was reported deleted")
	}
	if _, err := st.Get(c.ID); err != nil {
		t.Fatalf("running generation disappeared: %v", err)
	}
}

func TestSystemPruneSurfacesContainerCleanupFailureWithPartialProgress(t *testing.T) {
	st, _ := openContainerPruneTestStore(t)
	now := time.Now()
	saveStoppedPruneContainer(t, st, "a-prunable", now.Add(-2*time.Hour), false)
	saveStoppedPruneContainer(t, st, "z-pending-cgroup", now.Add(-2*time.Hour), true)

	res, err := SystemPrune(st, false)
	if err == nil {
		t.Fatal("SystemPrune unexpectedly hid pending container cleanup")
	}
	if res == nil || res.ContainersReclaimed != 1 {
		t.Fatalf("partial prune result=%+v, want one reclaimed container", res)
	}
	if !strings.Contains(err.Error(), "prune stopped container z-pending-cgroup") || !strings.Contains(err.Error(), "pending cgroup cleanup") {
		t.Fatalf("container prune error=%v", err)
	}
	if _, err := st.Get("a-prunable"); err == nil {
		t.Fatal("successfully pruned container still exists")
	}
	if _, err := st.Get("z-pending-cgroup"); err != nil {
		t.Fatalf("failed prune removed pending-cleanup container: %v", err)
	}
}

func TestPruneUntilSurfacesContainerCleanupFailure(t *testing.T) {
	st, _ := openContainerPruneTestStore(t)
	createdAt := time.Now().Add(-48 * time.Hour)
	saveStoppedPruneContainer(t, st, "old-pending-cgroup", createdAt, true)

	res, err := PruneUntil(st, time.Now().Add(-24*time.Hour))
	if err == nil {
		t.Fatal("PruneUntil unexpectedly hid pending container cleanup")
	}
	if res == nil || res.ContainersReclaimed != 0 {
		t.Fatalf("time-based prune result=%+v, want zero reclaimed", res)
	}
	if !strings.Contains(err.Error(), "prune stopped container old-pending-cgroup before cutoff") || !strings.Contains(err.Error(), "pending cgroup cleanup") {
		t.Fatalf("time-based prune error=%v", err)
	}
	if _, err := st.Get("old-pending-cgroup"); err != nil {
		t.Fatalf("failed time-based prune removed pending-cleanup container: %v", err)
	}
}
