//go:build linux

package container

import (
	"errors"
	"testing"
	"time"

	"minicontainer/internal/cgroups"
	"minicontainer/internal/state"
)

func TestApplyCgroupPersistsOwnershipBeforeHostMutation(t *testing.T) {
	st, err := state.Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	defer st.Close()

	const (
		id    = "ctr-cgroup-intent"
		pid   = 4242
		start = uint64(99)
	)
	if err := st.Save(&state.Container{
		ID: id, Status: state.StatusCreated, RootFS: "/tmp/rootfs",
		Command: []string{"true"}, CreatedAt: time.Now(),
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
	cfg := cgroups.Config{Name: name}

	called := false
	applied, err := applyCgroupWithDurableOwnership(st, id, pid, start, cfg, false, func(gotPID int, gotCfg cgroups.Config, _ bool) error {
		called = true
		ownership, ok, readErr := st.GetCgroupOwnership(id)
		if readErr != nil {
			t.Fatalf("read ownership inside apply: %v", readErr)
		}
		if !ok {
			t.Fatal("host mutation admitted before durable cgroup ownership existed")
		}
		if ownership.Name != name || ownership.PID != pid || ownership.PIDStartTime != start {
			t.Fatalf("ownership=%+v", ownership)
		}
		if gotPID != pid || gotCfg.Name != name {
			t.Fatalf("apply args pid=%d cfg=%+v", gotPID, gotCfg)
		}
		return nil
	})
	if err != nil {
		t.Fatal(err)
	}
	if !called || !applied {
		t.Fatalf("called=%v applied=%v", called, applied)
	}
}

func TestApplyCgroupFailureKeepsOwnershipForRecovery(t *testing.T) {
	st, err := state.Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	defer st.Close()

	const (
		id    = "ctr-cgroup-partial"
		pid   = 5252
		start = uint64(77)
	)
	if err := st.Save(&state.Container{
		ID: id, Status: state.StatusCreated, RootFS: "/tmp/rootfs",
		Command: []string{"true"}, CreatedAt: time.Now(),
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
	applyErr := errors.New("partial cgroup mutation")
	applied, err := applyCgroupWithDurableOwnership(st, id, pid, start, cgroups.Config{Name: name}, false, func(int, cgroups.Config, bool) error {
		return applyErr
	})
	if !errors.Is(err, applyErr) || applied {
		t.Fatalf("applied=%v err=%v", applied, err)
	}
	ownership, ok, readErr := st.GetCgroupOwnership(id)
	if readErr != nil {
		t.Fatal(readErr)
	}
	if !ok || ownership.Name != name {
		t.Fatalf("failed apply lost recovery ownership: %+v ok=%v", ownership, ok)
	}
}

func TestApplyCgroupOwnershipFailureBlocksHostMutation(t *testing.T) {
	st, err := state.Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	defer st.Close()

	const (
		id    = "ctr-cgroup-wrong-generation"
		pid   = 6262
		start = uint64(66)
	)
	if err := st.Save(&state.Container{
		ID: id, Status: state.StatusCreated, RootFS: "/tmp/rootfs",
		Command: []string{"true"}, CreatedAt: time.Now(),
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
	called := false
	applied, err := applyCgroupWithDurableOwnership(st, id, pid, start+1, cgroups.Config{Name: name}, false, func(int, cgroups.Config, bool) error {
		called = true
		return nil
	})
	if err == nil || !isRuntimeControlError(err) {
		t.Fatalf("expected runtime-control ownership failure, got %v", err)
	}
	if called || applied {
		t.Fatalf("host mutation ran without durable ownership: called=%v applied=%v", called, applied)
	}
}
