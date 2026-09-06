package container

import (
	"strings"
	"testing"
	"time"

	"minicontainer/internal/state"
)

func TestRenameContainerStopped(t *testing.T) {
	st, err := state.Open(t.TempDir())
	if err != nil {
		t.Fatalf("Open state store error: %v", err)
	}

	c := &state.Container{
		ID:        "ctr-ren-stopped",
		Hostname:  "old-name",
		Status:    state.StatusStopped,
		ExitCode:  17,
		CreatedAt: time.Now(),
	}
	if err := st.Save(c); err != nil {
		t.Fatal(err)
	}

	// Preserve the existing prefix-resolution behavior while mutating only the
	// canonical record selected by Resolve.
	if err := RenameContainer(st, "ctr-ren-st", "new-app-name"); err != nil {
		t.Fatalf("RenameContainer error: %v", err)
	}

	updated, err := st.Resolve(c.ID)
	if err != nil {
		t.Fatal(err)
	}
	if updated.Hostname != "new-app-name" {
		t.Fatalf("Hostname = %s, want new-app-name", updated.Hostname)
	}
	if updated.Status != state.StatusStopped || updated.ExitCode != 17 {
		t.Fatalf("lifecycle metadata changed: %+v", updated)
	}
}

func TestRenameContainerRejectsRunningGeneration(t *testing.T) {
	st, err := state.Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	c := &state.Container{
		ID:       "ctr-ren-running",
		Hostname: "old-name",
		Status:   state.StatusCreated,
	}
	if err := st.Save(c); err != nil {
		t.Fatal(err)
	}
	if err := st.MarkRunning(c.ID, 4242, 88, time.Unix(40, 0)); err != nil {
		t.Fatal(err)
	}

	err = RenameContainer(st, c.ID, "new-name")
	if err == nil || !strings.Contains(err.Error(), "running") {
		t.Fatalf("error=%v, want running refusal", err)
	}
	got, getErr := st.Get(c.ID)
	if getErr != nil {
		t.Fatal(getErr)
	}
	if got.Hostname != "old-name" || got.Status != state.StatusRunning || got.PID != 4242 || got.PIDStartTime != 88 {
		t.Fatalf("running generation changed: %+v", got)
	}
}
