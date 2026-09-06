package state

import (
	"strings"
	"testing"
	"time"
)

func TestDeleteIfNotRunningRejectsRunningGeneration(t *testing.T) {
	st, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	c := &Container{ID: "running-delete", Status: StatusRunning, PID: 4242, PIDStartTime: 88, CreatedAt: time.Now()}
	if err := st.Save(c); err != nil {
		t.Fatal(err)
	}
	err = st.DeleteIfNotRunning(c.ID)
	if err == nil || !strings.Contains(err.Error(), "refusing deletion") {
		t.Fatalf("delete running error=%v, want refusal", err)
	}
	got, getErr := st.Get(c.ID)
	if getErr != nil {
		t.Fatalf("running record disappeared: %v", getErr)
	}
	if got.Status != StatusRunning || got.PID != 4242 || got.PIDStartTime != 88 {
		t.Fatalf("running state changed after refused delete: %+v", got)
	}
}

func TestDeleteIfNotRunningAllowsStoppedAndCreated(t *testing.T) {
	for _, status := range []Status{StatusStopped, StatusCreated} {
		t.Run(string(status), func(t *testing.T) {
			st, err := Open(t.TempDir())
			if err != nil {
				t.Fatal(err)
			}
			id := "delete-" + string(status)
			if err := st.Save(&Container{ID: id, Status: status}); err != nil {
				t.Fatal(err)
			}
			if err := st.DeleteIfNotRunning(id); err != nil {
				t.Fatalf("delete %s: %v", status, err)
			}
			if _, err := st.Get(id); err == nil {
				t.Fatalf("%s record still exists after delete", status)
			}
		})
	}
}

func TestDeleteIfNotRunningRechecksLatestLifecycleState(t *testing.T) {
	dir := t.TempDir()
	observer, err := Open(dir)
	if err != nil {
		t.Fatal(err)
	}
	actor, err := Open(dir)
	if err != nil {
		t.Fatal(err)
	}
	c := &Container{ID: "restart-before-delete", Status: StatusStopped}
	if err := observer.Save(c); err != nil {
		t.Fatal(err)
	}
	if err := actor.MarkRunning(c.ID, 9001, 123, time.Now()); err != nil {
		t.Fatalf("restart: %v", err)
	}
	if err := observer.DeleteIfNotRunning(c.ID); err == nil {
		t.Fatal("stale stopped observer deleted a concurrently restarted container")
	}
	got, err := actor.Get(c.ID)
	if err != nil {
		t.Fatalf("restarted state disappeared: %v", err)
	}
	if got.Status != StatusRunning || got.PID != 9001 || got.PIDStartTime != 123 {
		t.Fatalf("restarted state changed: %+v", got)
	}
}

func TestDeleteIfNotRunningRemovesEmbeddedExitedIdentity(t *testing.T) {
	st, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	c := &Container{ID: "delete-exited-proof", Status: StatusRunning, PID: 5151, PIDStartTime: 91}
	if err := st.Save(c); err != nil {
		t.Fatal(err)
	}
	if changed, err := st.MarkStoppedIfIdentity(c.ID, c.PID, c.PIDStartTime, -1, time.Now()); err != nil || !changed {
		t.Fatalf("mark stopped unknown exit: changed=%v err=%v", changed, err)
	}
	if pid, start, ok, err := st.GetExitedIdentity(c.ID); err != nil || !ok || pid != c.PID || start != c.PIDStartTime {
		t.Fatalf("expected embedded exited identity before delete: pid=%d start=%d ok=%v err=%v", pid, start, ok, err)
	}
	if err := st.DeleteIfNotRunning(c.ID); err != nil {
		t.Fatalf("delete stopped container: %v", err)
	}
	if _, err := st.Get(c.ID); err == nil {
		t.Fatal("container JSON carrying embedded exit identity survived deletion")
	}
	if _, ok, err := st.readExitedIdentityUnlocked(c.ID); err != nil || ok {
		t.Fatalf("legacy sidecar appeared or survived delete: ok=%v err=%v", ok, err)
	}
}

func TestDeleteIfNotRunningPreservesPendingCgroupOwnership(t *testing.T) {
	st, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	c := &Container{ID: "pending-cgroup-delete", Status: StatusRunning, PID: 6161, PIDStartTime: 101}
	if err := st.Save(c); err != nil {
		t.Fatal(err)
	}
	name := "minicontainer-pending-cgroup-delete-6161-101"
	if err := st.MarkCgroupOwnedIfIdentity(c.ID, c.PID, c.PIDStartTime, name); err != nil {
		t.Fatalf("mark cgroup ownership: %v", err)
	}
	if changed, err := st.MarkStoppedIfIdentity(c.ID, c.PID, c.PIDStartTime, -1, time.Now()); err != nil || !changed {
		t.Fatalf("mark stopped: changed=%v err=%v", changed, err)
	}
	err = st.DeleteIfNotRunning(c.ID)
	if err == nil || !strings.Contains(err.Error(), "pending cgroup cleanup") {
		t.Fatalf("delete with pending cgroup error=%v, want cleanup refusal", err)
	}
	if _, err := st.Get(c.ID); err != nil {
		t.Fatalf("container record disappeared despite pending cleanup: %v", err)
	}
	ownership, ok, err := st.GetCgroupOwnership(c.ID)
	if err != nil {
		t.Fatalf("read cgroup ownership: %v", err)
	}
	if !ok || ownership.Name != name || ownership.PID != c.PID || ownership.PIDStartTime != c.PIDStartTime {
		t.Fatalf("pending ownership changed after refused delete: ok=%v ownership=%+v", ok, ownership)
	}
}
