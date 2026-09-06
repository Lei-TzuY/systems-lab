package state

import (
	"strings"
	"testing"
	"time"
)

func TestSetHostnameIfNotRunningUpdatesStoppedRecord(t *testing.T) {
	st, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	c := &Container{
		ID:         "rename-stopped",
		Status:     StatusStopped,
		Hostname:   "old-name",
		ExitCode:   17,
		FinishedAt: ptrTime(time.Unix(20, 0)),
	}
	if err := st.Save(c); err != nil {
		t.Fatal(err)
	}
	before := c.Revision

	if err := st.SetHostnameIfNotRunning(c.ID, "new-name"); err != nil {
		t.Fatalf("set hostname: %v", err)
	}
	got, err := st.Get(c.ID)
	if err != nil {
		t.Fatal(err)
	}
	if got.Hostname != "new-name" {
		t.Fatalf("hostname=%q, want new-name", got.Hostname)
	}
	if got.Status != StatusStopped || got.ExitCode != 17 || got.FinishedAt == nil || !got.FinishedAt.Equal(time.Unix(20, 0)) {
		t.Fatalf("non-hostname lifecycle fields changed: %+v", got)
	}
	if got.Revision != before+1 {
		t.Fatalf("revision=%d, want %d", got.Revision, before+1)
	}
}

func TestSetHostnameIfNotRunningRejectsRunningRecord(t *testing.T) {
	st, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	c := &Container{ID: "rename-running", Status: StatusCreated, Hostname: "old-name"}
	if err := st.Save(c); err != nil {
		t.Fatal(err)
	}
	if err := st.MarkRunning(c.ID, 4242, 88, time.Unix(30, 0)); err != nil {
		t.Fatal(err)
	}
	before, err := st.Get(c.ID)
	if err != nil {
		t.Fatal(err)
	}

	err = st.SetHostnameIfNotRunning(c.ID, "new-name")
	if err == nil || !strings.Contains(err.Error(), "running") {
		t.Fatalf("error=%v, want running refusal", err)
	}
	got, getErr := st.Get(c.ID)
	if getErr != nil {
		t.Fatal(getErr)
	}
	if got.Hostname != "old-name" || got.Status != StatusRunning || got.PID != 4242 || got.PIDStartTime != 88 {
		t.Fatalf("running record changed: %+v", got)
	}
	if got.Revision != before.Revision {
		t.Fatalf("revision=%d, want unchanged %d", got.Revision, before.Revision)
	}
}

func TestSetHostnameIfNotRunningRejectsUnknownStatus(t *testing.T) {
	st, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	c := &Container{ID: "rename-unknown", Status: Status("future"), Hostname: "old-name"}
	if err := st.Save(c); err != nil {
		t.Fatal(err)
	}
	before := c.Revision

	err = st.SetHostnameIfNotRunning(c.ID, "new-name")
	if err == nil || !strings.Contains(err.Error(), "unknown lifecycle status") {
		t.Fatalf("error=%v, want unknown-status refusal", err)
	}
	got, getErr := st.Get(c.ID)
	if getErr != nil {
		t.Fatal(getErr)
	}
	if got.Hostname != "old-name" || got.Status != Status("future") || got.Revision != before {
		t.Fatalf("unknown-status record changed: %+v", got)
	}
}

func TestSetHostnameIfNotRunningIsIdempotentForSameHostname(t *testing.T) {
	st, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	c := &Container{ID: "rename-idempotent", Status: StatusCreated, Hostname: "same-name"}
	if err := st.Save(c); err != nil {
		t.Fatal(err)
	}
	before := c.Revision

	if err := st.SetHostnameIfNotRunning(c.ID, "same-name"); err != nil {
		t.Fatalf("same hostname: %v", err)
	}
	got, err := st.Get(c.ID)
	if err != nil {
		t.Fatal(err)
	}
	if got.Revision != before {
		t.Fatalf("revision=%d, want unchanged %d", got.Revision, before)
	}
}

func ptrTime(v time.Time) *time.Time { return &v }
