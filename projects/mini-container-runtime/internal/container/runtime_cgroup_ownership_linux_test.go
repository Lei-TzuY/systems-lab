//go:build linux

package container

import (
	"errors"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"minicontainer/internal/cgroups"
	"minicontainer/internal/state"
)

func saveOwnershipTestRunning(t *testing.T, st *state.Store, id string, pid int, start uint64) string {
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
	name, err := cgroups.NameForContainerProcess(id, pid, start)
	if err != nil {
		t.Fatal(err)
	}
	return name
}

func TestPersistAppliedCgroupOwnershipDurableBeforeRelease(t *testing.T) {
	st, err := state.Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	defer st.Close()
	const (
		id    = "ctr-applied-ownership"
		pid   = 4242
		start = uint64(99)
	)
	name := saveOwnershipTestRunning(t, st, id, pid, start)

	// A nil cmd/pipe is intentional: the success path must only persist the
	// ownership token and must not touch the blocked child or cleanup path.
	if err := persistAppliedCgroupOwnership(nil, nil, st, id, pid, start, name, false); err != nil {
		t.Fatalf("persist applied ownership: %v", err)
	}
	ownership, ok, err := st.GetCgroupOwnership(id)
	if err != nil {
		t.Fatal(err)
	}
	if !ok || ownership.Name != name || ownership.PID != pid || ownership.PIDStartTime != start {
		t.Fatalf("ownership=%+v ok=%v", ownership, ok)
	}
}

func TestPersistAppliedCgroupOwnershipFailureStopsConfirmedReapedGeneration(t *testing.T) {
	dir := t.TempDir()
	st, err := state.Open(dir)
	if err != nil {
		t.Fatal(err)
	}
	defer st.Close()
	const (
		id    = "ctr-applied-persist-fail"
		pid   = 5252
		start = uint64(77)
	)
	name := saveOwnershipTestRunning(t, st, id, pid, start)

	// Block the sidecar target with a directory. atomic rename cannot replace a
	// directory, forcing ownership persistence to fail after Apply would have
	// succeeded. Once abort has authoritatively reaped the child, lifecycle state
	// may transition to stopped and the known-owned cgroup may be removed.
	sidecar := filepath.Join(dir, "containers", id+".cgroup")
	if err := os.Mkdir(sidecar, 0o700); err != nil {
		t.Fatal(err)
	}

	err = persistAppliedCgroupOwnershipWithAbort(
		nil,
		nil,
		st,
		id,
		pid,
		start,
		name,
		false,
		func(_ *exec.Cmd, _ *os.File) (bool, error) { return true, nil },
	)
	if err == nil {
		t.Fatal("ownership persistence failure unexpectedly succeeded")
	}
	if !isRuntimeControlError(err) {
		t.Fatalf("ownership persistence failure not classified runtime-control: %v", err)
	}
	if !strings.Contains(err.Error(), "persist cgroup ownership") {
		t.Fatalf("missing ownership persistence context: %v", err)
	}

	current, getErr := st.Get(id)
	if getErr != nil {
		t.Fatal(getErr)
	}
	if current.Status != state.StatusStopped || current.PID != 0 || current.PIDStartTime != 0 {
		t.Fatalf("confirmed-reaped generation not stopped after ownership persistence failure: %+v", current)
	}
}

func TestPersistAppliedCgroupOwnershipFailurePreservesRunningWithoutReapProof(t *testing.T) {
	dir := t.TempDir()
	st, err := state.Open(dir)
	if err != nil {
		t.Fatal(err)
	}
	defer st.Close()
	const (
		id    = "ctr-applied-unreaped"
		pid   = 6262
		start = uint64(88)
	)
	name := saveOwnershipTestRunning(t, st, id, pid, start)

	sidecar := filepath.Join(dir, "containers", id+".cgroup")
	if err := os.Mkdir(sidecar, 0o700); err != nil {
		t.Fatal(err)
	}

	abortErr := errors.New("kill denied")
	err = persistAppliedCgroupOwnershipWithAbort(
		nil,
		nil,
		st,
		id,
		pid,
		start,
		name,
		false,
		func(_ *exec.Cmd, _ *os.File) (bool, error) { return false, abortErr },
	)
	if err == nil {
		t.Fatal("unreaped ownership persistence failure unexpectedly succeeded")
	}
	if !errors.Is(err, abortErr) {
		t.Fatalf("error=%v, want abort cause", err)
	}
	if !strings.Contains(err.Error(), "not confirmed reaped") {
		t.Fatalf("error=%v, want explicit reap-proof failure", err)
	}
	if !isRuntimeControlError(err) {
		t.Fatalf("unreaped failure not classified runtime-control: %v", err)
	}

	current, getErr := st.Get(id)
	if getErr != nil {
		t.Fatal(getErr)
	}
	if current.Status != state.StatusRunning || current.PID != pid || current.PIDStartTime != start {
		t.Fatalf("unreaped generation lifecycle changed: %+v", current)
	}
	if current.FinishedAt != nil {
		t.Fatalf("unreaped generation gained FinishedAt=%v", current.FinishedAt)
	}
}
