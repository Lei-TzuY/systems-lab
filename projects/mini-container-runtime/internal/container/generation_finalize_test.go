package container

import (
	"errors"
	"testing"
	"time"

	"minicontainer/internal/cgroups"
	"minicontainer/internal/state"
)

func persistOwnedGeneration(t *testing.T, st *state.Store, snapshot *state.Container) state.CgroupOwnership {
	t.Helper()
	if err := st.Save(snapshot); err != nil {
		t.Fatal(err)
	}
	name, err := cgroups.NameForContainerProcess(snapshot.ID, snapshot.PID, snapshot.PIDStartTime)
	if err != nil {
		t.Fatal(err)
	}
	if err := st.MarkCgroupOwnedIfIdentity(snapshot.ID, snapshot.PID, snapshot.PIDStartTime, name); err != nil {
		t.Fatal(err)
	}
	return state.CgroupOwnership{Name: name, PID: snapshot.PID, PIDStartTime: snapshot.PIDStartTime}
}

func TestFinalizeStoppedGenerationPersistsStateAndOwnershipWhenCleanupFails(t *testing.T) {
	st, err := state.Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}

	snapshot := &state.Container{
		ID:           "ctr-finalize-cleanup-error",
		Status:       state.StatusRunning,
		PID:          4242,
		PIDStartTime: 99,
		CreatedAt:    time.Now(),
	}
	ownership := persistOwnedGeneration(t, st, snapshot)

	cleanupFailure := errors.New("cgroup still populated")
	var gotID string
	var gotPID int
	var gotStart uint64
	changed, err := finalizeStoppedGenerationWithCleanup(
		st,
		snapshot,
		-1,
		time.Now(),
		func(id string, pid int, start uint64) error {
			gotID, gotPID, gotStart = id, pid, start
			return cleanupFailure
		},
	)
	if !changed {
		t.Fatal("expected running state to transition to stopped")
	}
	if !errors.Is(err, cleanupFailure) {
		t.Fatalf("cleanup failure not preserved: %v", err)
	}
	if gotID != snapshot.ID || gotPID != snapshot.PID || gotStart != snapshot.PIDStartTime {
		t.Fatalf("cleanup targeted wrong generation: id=%q pid=%d start=%d", gotID, gotPID, gotStart)
	}

	current, err := st.Get(snapshot.ID)
	if err != nil {
		t.Fatal(err)
	}
	if current.Status != state.StatusStopped || current.PID != 0 || current.PIDStartTime != 0 {
		t.Fatalf("dead process left as running after cleanup failure: %+v", current)
	}
	gotOwnership, ok, err := st.GetCgroupOwnership(snapshot.ID)
	if err != nil {
		t.Fatal(err)
	}
	if !ok || gotOwnership != ownership {
		t.Fatalf("cleanup failure lost retry proof: ownership=%+v ok=%v", gotOwnership, ok)
	}
}

func TestFinalizeStoppedGenerationClearsOwnershipOnlyAfterCleanupSuccess(t *testing.T) {
	st, err := state.Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	snapshot := &state.Container{
		ID:           "ctr-finalize-clean",
		Status:       state.StatusRunning,
		PID:          1234,
		PIDStartTime: 55,
		CreatedAt:    time.Now(),
	}
	persistOwnedGeneration(t, st, snapshot)

	cleanupCalls := 0
	changed, err := finalizeStoppedGenerationWithCleanup(
		st,
		snapshot,
		7,
		time.Now(),
		func(id string, pid int, start uint64) error {
			cleanupCalls++
			if id != snapshot.ID || pid != snapshot.PID || start != snapshot.PIDStartTime {
				t.Fatalf("wrong cleanup generation: %s %d/%d", id, pid, start)
			}
			return nil
		},
	)
	if err != nil {
		t.Fatalf("finalize owned generation: %v", err)
	}
	if !changed || cleanupCalls != 1 {
		t.Fatalf("changed=%v cleanupCalls=%d, want true/1", changed, cleanupCalls)
	}
	if _, ok, err := st.GetCgroupOwnership(snapshot.ID); err != nil || ok {
		t.Fatalf("successful cleanup retained ownership: ok=%v err=%v", ok, err)
	}
}

func TestFinalizeStoppedGenerationLegacyWithoutOwnershipNeverDerivesCleanup(t *testing.T) {
	st, err := state.Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	snapshot := &state.Container{
		ID:           "ctr-finalize-legacy",
		Status:       state.StatusRunning,
		PID:          88,
		PIDStartTime: 99,
		CreatedAt:    time.Now(),
	}
	if err := st.Save(snapshot); err != nil {
		t.Fatal(err)
	}

	cleanupCalls := 0
	changed, err := finalizeStoppedGenerationWithCleanup(
		st,
		snapshot,
		-1,
		time.Now(),
		func(string, int, uint64) error {
			cleanupCalls++
			return errors.New("must not infer ownership")
		},
	)
	if err != nil {
		t.Fatalf("finalize legacy generation: %v", err)
	}
	if !changed {
		t.Fatal("legacy running state was not reconciled")
	}
	if cleanupCalls != 0 {
		t.Fatalf("legacy generation cleanup calls=%d, want 0", cleanupCalls)
	}
}

func TestFinalizeStoppedGenerationDoesNotClobberConcurrentRestartWithoutOldOwnership(t *testing.T) {
	st, err := state.Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}

	oldGeneration := &state.Container{
		ID:           "ctr-finalize-restart",
		Status:       state.StatusRunning,
		PID:          1111,
		PIDStartTime: 10,
		CreatedAt:    time.Now(),
	}
	newGeneration := &state.Container{
		ID:           oldGeneration.ID,
		Status:       state.StatusRunning,
		PID:          2222,
		PIDStartTime: 20,
		CreatedAt:    oldGeneration.CreatedAt,
	}
	if err := st.Save(newGeneration); err != nil {
		t.Fatal(err)
	}

	cleanupCalls := 0
	changed, err := finalizeStoppedGenerationWithCleanup(
		st,
		oldGeneration,
		-1,
		time.Now(),
		func(string, int, uint64) error {
			cleanupCalls++
			return nil
		},
	)
	if err != nil {
		t.Fatalf("finalize stale old generation: %v", err)
	}
	if changed {
		t.Fatal("stale finalizer overwrote concurrently restarted state")
	}
	if cleanupCalls != 0 {
		t.Fatalf("stale generation without durable ownership was cleaned %d time(s)", cleanupCalls)
	}

	current, err := st.Get(oldGeneration.ID)
	if err != nil {
		t.Fatal(err)
	}
	if current.Status != state.StatusRunning || current.PID != newGeneration.PID || current.PIDStartTime != newGeneration.PIDStartTime {
		t.Fatalf("restart state was clobbered: %+v", current)
	}
}

func TestCleanupStoppedCgroupRetriesPersistedOwnership(t *testing.T) {
	st, err := state.Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	snapshot := &state.Container{
		ID:           "ctr-cleanup-retry",
		Status:       state.StatusRunning,
		PID:          313,
		PIDStartTime: 414,
		CreatedAt:    time.Now(),
	}
	ownership := persistOwnedGeneration(t, st, snapshot)
	if _, err := st.MarkStoppedIfIdentity(snapshot.ID, snapshot.PID, snapshot.PIDStartTime, -1, time.Now()); err != nil {
		t.Fatal(err)
	}
	stopped, err := st.Get(snapshot.ID)
	if err != nil {
		t.Fatal(err)
	}

	calls := 0
	if err := cleanupStoppedCgroupWithCleanup(st, stopped, func(id string, pid int, start uint64) error {
		calls++
		if id != snapshot.ID || pid != ownership.PID || start != ownership.PIDStartTime {
			t.Fatalf("retry targeted wrong ownership: %s %d/%d", id, pid, start)
		}
		return nil
	}); err != nil {
		t.Fatalf("retry stopped cleanup: %v", err)
	}
	if calls != 1 {
		t.Fatalf("retry cleanup calls=%d, want 1", calls)
	}
	if _, ok, err := st.GetCgroupOwnership(snapshot.ID); err != nil || ok {
		t.Fatalf("retry did not clear ownership: ok=%v err=%v", ok, err)
	}
}
