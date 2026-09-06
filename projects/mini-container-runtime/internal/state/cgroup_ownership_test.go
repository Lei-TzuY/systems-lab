package state

import (
	"os"
	"strings"
	"testing"
	"time"
)

func saveCreatedContainer(t *testing.T, st *Store, id string) {
	t.Helper()
	if err := st.Save(&Container{
		ID:        id,
		Status:    StatusCreated,
		RootFS:    "/tmp/rootfs",
		Command:   []string{"true"},
		CreatedAt: time.Now(),
	}); err != nil {
		t.Fatal(err)
	}
}

func TestCgroupOwnershipPersistsAcrossStopAndClearsExactly(t *testing.T) {
	st, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	const id = "ctr-cgroup-owner"
	saveCreatedContainer(t, st, id)
	if err := st.MarkRunning(id, 101, 202, time.Now()); err != nil {
		t.Fatal(err)
	}
	ownership := CgroupOwnership{Name: "minicontainer-ctr-cgroup-owner-101-202", PID: 101, PIDStartTime: 202}
	if err := st.MarkCgroupOwnedIfIdentity(id, ownership.PID, ownership.PIDStartTime, ownership.Name); err != nil {
		t.Fatal(err)
	}
	if err := st.MarkCgroupOwnedIfIdentity(id, ownership.PID, ownership.PIDStartTime, ownership.Name); err != nil {
		t.Fatalf("idempotent ownership write: %v", err)
	}
	if _, err := st.MarkStoppedIfIdentity(id, ownership.PID, ownership.PIDStartTime, -1, time.Now()); err != nil {
		t.Fatal(err)
	}
	got, ok, err := st.GetCgroupOwnership(id)
	if err != nil || !ok || got != ownership {
		t.Fatalf("stop lost ownership: got=%+v ok=%v err=%v", got, ok, err)
	}

	wrong := ownership
	wrong.PID++
	if changed, err := st.ClearCgroupOwnershipIfMatch(id, wrong); err != nil || changed {
		t.Fatalf("stale clear changed=%v err=%v", changed, err)
	}
	if changed, err := st.ClearCgroupOwnershipIfMatch(id, ownership); err != nil || !changed {
		t.Fatalf("matching clear changed=%v err=%v", changed, err)
	}
	if _, ok, err := st.GetCgroupOwnership(id); err != nil || ok {
		t.Fatalf("ownership remains after clear: ok=%v err=%v", ok, err)
	}
}

func TestPendingCgroupOwnershipBlocksRestartAndDelete(t *testing.T) {
	st, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	const id = "ctr-cgroup-pending"
	saveCreatedContainer(t, st, id)
	if err := st.MarkRunning(id, 41, 42, time.Now()); err != nil {
		t.Fatal(err)
	}
	ownership := CgroupOwnership{Name: "minicontainer-ctr-cgroup-pending-41-42", PID: 41, PIDStartTime: 42}
	if err := st.MarkCgroupOwnedIfIdentity(id, ownership.PID, ownership.PIDStartTime, ownership.Name); err != nil {
		t.Fatal(err)
	}
	if _, err := st.MarkStoppedIfIdentity(id, ownership.PID, ownership.PIDStartTime, -1, time.Now()); err != nil {
		t.Fatal(err)
	}
	if err := st.MarkRunning(id, 51, 52, time.Now()); err == nil || !strings.Contains(err.Error(), "pending cgroup cleanup") {
		t.Fatalf("restart with pending cleanup error=%v", err)
	}
	if err := st.Delete(id); err == nil || !strings.Contains(err.Error(), "pending cgroup cleanup") {
		t.Fatalf("delete with pending cleanup error=%v", err)
	}
	if changed, err := st.ClearCgroupOwnershipIfMatch(id, ownership); err != nil || !changed {
		t.Fatalf("clear pending ownership changed=%v err=%v", changed, err)
	}
	if err := st.MarkRunning(id, 51, 52, time.Now()); err != nil {
		t.Fatalf("restart after cleanup: %v", err)
	}
}

func TestCgroupOwnershipRequiresExactRunningIdentity(t *testing.T) {
	st, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	const id = "ctr-cgroup-identity"
	saveCreatedContainer(t, st, id)
	if err := st.MarkRunning(id, 111, 222, time.Now()); err != nil {
		t.Fatal(err)
	}
	if err := st.MarkCgroupOwnedIfIdentity(id, 111, 333, "minicontainer-ctr-cgroup-identity-111-333"); err == nil || !strings.Contains(err.Error(), "not bound") {
		t.Fatalf("wrong identity ownership error=%v", err)
	}
	if _, ok, err := st.GetCgroupOwnership(id); err != nil || ok {
		t.Fatalf("wrong identity persisted ownership: ok=%v err=%v", ok, err)
	}
}

func TestClearCgroupOwnershipRefusesRunningContainer(t *testing.T) {
	st, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	const id = "ctr-cgroup-running-clear"
	saveCreatedContainer(t, st, id)
	if err := st.MarkRunning(id, 11, 22, time.Now()); err != nil {
		t.Fatal(err)
	}
	ownership := CgroupOwnership{Name: "minicontainer-ctr-cgroup-running-clear-11-22", PID: 11, PIDStartTime: 22}
	if err := st.MarkCgroupOwnedIfIdentity(id, ownership.PID, ownership.PIDStartTime, ownership.Name); err != nil {
		t.Fatal(err)
	}
	if changed, err := st.ClearCgroupOwnershipIfMatch(id, ownership); err == nil || changed || !strings.Contains(err.Error(), "running container") {
		t.Fatalf("running clear changed=%v err=%v", changed, err)
	}
	if _, ok, err := st.GetCgroupOwnership(id); err != nil || !ok {
		t.Fatalf("running ownership disappeared: ok=%v err=%v", ok, err)
	}
}

func TestCgroupOwnershipRejectsValidJSONWithUnsafeName(t *testing.T) {
	st, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	const id = "ctr-cgroup-corrupt"
	saveCreatedContainer(t, st, id)
	path := cgroupOwnershipPath(st.ctrDir, id)
	if err := os.WriteFile(path, []byte(`{"name":"../escape","pid":1,"pid_start_time":2}`), 0o600); err != nil {
		t.Fatal(err)
	}
	_, ok, err := st.GetCgroupOwnership(id)
	if err == nil || !strings.Contains(err.Error(), "invalid persisted cgroup ownership") {
		t.Fatalf("unsafe sidecar error=%v", err)
	}
	if ok {
		t.Fatal("unsafe sidecar reported as valid ownership")
	}
}

func TestCgroupOwnershipSidecarPrivateAndSymlinkReadRejected(t *testing.T) {
	st, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	const id = "ctr-cgroup-private"
	saveCreatedContainer(t, st, id)
	if err := st.MarkRunning(id, 7, 8, time.Now()); err != nil {
		t.Fatal(err)
	}
	name := "minicontainer-ctr-cgroup-private-7-8"
	if err := st.MarkCgroupOwnedIfIdentity(id, 7, 8, name); err != nil {
		t.Fatal(err)
	}
	path := cgroupOwnershipPath(st.ctrDir, id)
	info, err := os.Lstat(path)
	if err != nil {
		t.Fatal(err)
	}
	if !info.Mode().IsRegular() || info.Mode().Perm() != 0o600 {
		t.Fatalf("ownership sidecar mode=%v perm=%#o", info.Mode(), info.Mode().Perm())
	}

	if _, err := st.MarkStoppedIfIdentity(id, 7, 8, -1, time.Now()); err != nil {
		t.Fatal(err)
	}
	ownership, ok, err := st.GetCgroupOwnership(id)
	if err != nil || !ok {
		t.Fatalf("read ownership before symlink replacement: ok=%v err=%v", ok, err)
	}
	if changed, err := st.ClearCgroupOwnershipIfMatch(id, ownership); err != nil || !changed {
		t.Fatalf("clear ownership before symlink test changed=%v err=%v", changed, err)
	}
	outside := t.TempDir() + "/ownership"
	if err := os.WriteFile(outside, []byte(`{"name":"minicontainer-safe","pid":1,"pid_start_time":2}`), 0o600); err != nil {
		t.Fatal(err)
	}
	if err := os.Symlink(outside, path); err != nil {
		t.Fatal(err)
	}
	if _, _, err := st.GetCgroupOwnership(id); err == nil {
		t.Fatal("symlinked cgroup ownership sidecar was followed")
	}
}
