package state

import (
	"testing"
	"time"
)

func TestLifecycleAllowsRestartAfterMatchingStop(t *testing.T) {
	st, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	if err := st.Save(&Container{ID: "ctr-restart", Status: StatusCreated, CreatedAt: time.Now()}); err != nil {
		t.Fatal(err)
	}
	if err := st.MarkRunning("ctr-restart", 10, 100, time.Now()); err != nil {
		t.Fatal(err)
	}
	if changed, err := st.MarkStoppedIfIdentity("ctr-restart", 10, 100, 1, time.Now()); err != nil || !changed {
		t.Fatalf("first stop changed=%v err=%v", changed, err)
	}
	if err := st.MarkRunning("ctr-restart", 11, 200, time.Now()); err != nil {
		t.Fatalf("restart MarkRunning: %v", err)
	}

	current, err := st.Get("ctr-restart")
	if err != nil {
		t.Fatal(err)
	}
	if current.Status != StatusRunning || current.PID != 11 || current.PIDStartTime != 200 {
		t.Fatalf("unexpected restarted state: %+v", current)
	}
}
