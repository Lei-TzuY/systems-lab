package state

import (
	"testing"
	"time"
)

func TestExitedIdentityForStoppedRevisionRejectsPreStopSidecar(t *testing.T) {
	const (
		id    = "ctr-prestop-identity"
		pid   = 7101
		start = 8101
	)
	st, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	if err := st.Save(&Container{ID: id, Status: StatusCreated, CreatedAt: time.Now()}); err != nil {
		t.Fatal(err)
	}
	if err := st.MarkRunning(id, pid, start, time.Now()); err != nil {
		t.Fatal(err)
	}
	running, err := st.Get(id)
	if err != nil {
		t.Fatal(err)
	}

	// Model a crash after the durable .exit write but before the stopped state
	// commit. The sidecar must not become teardown authority while lifecycle
	// state still says this exact process generation is running.
	if err := st.writeExitedIdentityUnlocked(id, pid, start); err != nil {
		t.Fatal(err)
	}
	gotPID, gotStart, current, ok, err := st.GetExitedIdentityForStoppedRevision(id, running.Revision)
	if err != nil {
		t.Fatal(err)
	}
	if current || ok || gotPID != 0 || gotStart != 0 {
		t.Fatalf("pre-stop sidecar became authoritative: pid=%d start=%d current=%v ok=%v", gotPID, gotStart, current, ok)
	}

	if changed, err := st.MarkStoppedIfIdentity(id, pid, start, 0, time.Now()); err != nil || !changed {
		t.Fatalf("finish stop after simulated crash window: changed=%v err=%v", changed, err)
	}
	stopped, err := st.Get(id)
	if err != nil {
		t.Fatal(err)
	}
	gotPID, gotStart, current, ok, err = st.GetExitedIdentityForStoppedRevision(id, stopped.Revision)
	if err != nil || !current || !ok || gotPID != pid || gotStart != start {
		t.Fatalf("committed stopped identity unavailable: pid=%d start=%d current=%v ok=%v err=%v", gotPID, gotStart, current, ok, err)
	}
}

func TestExitedIdentityForStoppedRevisionRejectsOldStoppedGeneration(t *testing.T) {
	const id = "ctr-stale-stopped-identity"
	st, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	if err := st.Save(&Container{ID: id, Status: StatusCreated, CreatedAt: time.Now()}); err != nil {
		t.Fatal(err)
	}
	if err := st.MarkRunning(id, 7201, 8201, time.Now()); err != nil {
		t.Fatal(err)
	}
	if changed, err := st.MarkStoppedIfIdentity(id, 7201, 8201, 0, time.Now()); err != nil || !changed {
		t.Fatalf("stop generation A: changed=%v err=%v", changed, err)
	}
	stoppedA, err := st.Get(id)
	if err != nil {
		t.Fatal(err)
	}

	if err := st.MarkRunning(id, 7202, 8202, time.Now()); err != nil {
		t.Fatal(err)
	}
	if changed, err := st.MarkStoppedIfIdentity(id, 7202, 8202, 0, time.Now()); err != nil || !changed {
		t.Fatalf("stop generation B: changed=%v err=%v", changed, err)
	}
	stoppedB, err := st.Get(id)
	if err != nil {
		t.Fatal(err)
	}

	if pid, start, current, ok, err := st.GetExitedIdentityForStoppedRevision(id, stoppedA.Revision); err != nil {
		t.Fatal(err)
	} else if current || ok || pid != 0 || start != 0 {
		t.Fatalf("old stopped generation retained teardown authority: pid=%d start=%d current=%v ok=%v", pid, start, current, ok)
	}
	if pid, start, current, ok, err := st.GetExitedIdentityForStoppedRevision(id, stoppedB.Revision); err != nil {
		t.Fatal(err)
	} else if !current || !ok || pid != 7202 || start != 8202 {
		t.Fatalf("current stopped generation lost teardown authority: pid=%d start=%d current=%v ok=%v", pid, start, current, ok)
	}
}
