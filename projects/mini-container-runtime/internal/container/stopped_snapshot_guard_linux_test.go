//go:build linux

package container

import (
	"testing"
	"time"

	"minicontainer/internal/cgroups"
	"minicontainer/internal/state"
)

func stoppedSnapshotBeforeRestart(t *testing.T, st *state.Store, id string, pid int, start uint64) *state.Container {
	t.Helper()
	if err := st.Save(&state.Container{
		ID:        id,
		Status:    state.StatusCreated,
		RootFS:    "/tmp/rootfs",
		Command:   []string{"true"},
		CreatedAt: time.Now(),
	}); err != nil {
		t.Fatal(err)
	}
	if err := st.MarkRunning(id, pid, start, time.Now()); err != nil {
		t.Fatal(err)
	}
	if changed, err := st.MarkStoppedIfIdentity(id, pid, start, 0, time.Now()); err != nil || !changed {
		t.Fatalf("stop first generation: changed=%v err=%v", changed, err)
	}
	snapshot, err := st.Get(id)
	if err != nil {
		t.Fatal(err)
	}
	return snapshot
}

func TestCleanupStoppedCgroupRejectsSnapshotFromEarlierStoppedGeneration(t *testing.T) {
	st, err := state.Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	const id = "ctr-stale-cgroup-cleanup"
	stale := stoppedSnapshotBeforeRestart(t, st, id, 101, 1001)

	const newPID = 202
	const newStart uint64 = 2002
	if err := st.MarkRunning(id, newPID, newStart, time.Now()); err != nil {
		t.Fatal(err)
	}
	name, err := cgroups.NameForContainerProcess(id, newPID, newStart)
	if err != nil {
		t.Fatal(err)
	}
	if err := st.MarkCgroupOwnedIfIdentity(id, newPID, newStart, name); err != nil {
		t.Fatal(err)
	}
	if changed, err := st.MarkStoppedIfIdentity(id, newPID, newStart, 0, time.Now()); err != nil || !changed {
		t.Fatalf("stop second generation: changed=%v err=%v", changed, err)
	}

	cleanupCalls := 0
	if err := cleanupStoppedCgroupWithCleanup(st, stale, func(string, int, uint64) error {
		cleanupCalls++
		return nil
	}); err != nil {
		t.Fatalf("stale cgroup cleanup: %v", err)
	}
	if cleanupCalls != 0 {
		t.Fatalf("stale stopped snapshot cleaned newer cgroup %d time(s)", cleanupCalls)
	}
	ownership, ok, err := st.GetCgroupOwnership(id)
	if err != nil || !ok {
		t.Fatalf("newer cgroup ownership lost: ok=%v err=%v", ok, err)
	}
	if ownership.PID != newPID || ownership.PIDStartTime != newStart {
		t.Fatalf("wrong cgroup ownership survived: %+v", ownership)
	}
}

func TestCleanupStoppedNetworkRejectsSnapshotFromEarlierStoppedGeneration(t *testing.T) {
	st, err := state.Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	const id = "ctr-stale-network-cleanup"
	stale := stoppedSnapshotBeforeRestart(t, st, id, 303, 3003)

	const newPID = 404
	const newStart uint64 = 4004
	if err := st.MarkRunning(id, newPID, newStart, time.Now()); err != nil {
		t.Fatal(err)
	}
	ownership := networkOwnershipForGeneration("minicontainer:new-generation", newPID, newStart, "172.20.0.2", nil)
	if err := st.MarkNetworkOwnedIfIdentity(id, ownership); err != nil {
		t.Fatal(err)
	}
	if changed, err := st.MarkStoppedIfIdentity(id, newPID, newStart, 0, time.Now()); err != nil || !changed {
		t.Fatalf("stop second generation: changed=%v err=%v", changed, err)
	}

	if err := CleanupStoppedNetwork(st, stale); err != nil {
		t.Fatalf("stale network cleanup: %v", err)
	}
	got, ok, err := st.GetNetworkOwnership(id)
	if err != nil || !ok {
		t.Fatalf("newer network ownership lost: ok=%v err=%v", ok, err)
	}
	if got.PID != newPID || got.PIDStartTime != newStart || got.Owner != ownership.Owner {
		t.Fatalf("newer network ownership changed: %+v", got)
	}
}

func TestStoppedSnapshotGuardAcceptsCurrentStoppedRevision(t *testing.T) {
	st, err := state.Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	current := stoppedSnapshotBeforeRestart(t, st, "ctr-current-stopped", 505, 5005)
	ok, err := stoppedSnapshotStillCurrent(st, current)
	if err != nil {
		t.Fatal(err)
	}
	if !ok {
		t.Fatal("current stopped snapshot was rejected")
	}
}
