//go:build linux

package container

import (
	"errors"
	"os"
	"os/exec"
	"strings"
	"testing"
	"time"

	"minicontainer/internal/state"
)

func TestAbortRuntimeSetupFailurePreservesRunningStateWithoutReapProof(t *testing.T) {
	st, err := state.Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	defer st.Close()

	const (
		id    = "ctr-unreaped-setup-abort"
		pid   = 424242
		start = uint64(9191)
	)
	if err := st.Save(&state.Container{
		ID:        id,
		Status:    state.StatusCreated,
		RootFS:    "/tmp/rootfs",
		Command:   []string{"sleep", "30"},
		CreatedAt: time.Now(),
	}); err != nil {
		t.Fatal(err)
	}
	if err := st.MarkRunning(id, pid, start, time.Now()); err != nil {
		t.Fatal(err)
	}

	setupCause := errors.New("required setup failed")
	abortCause := errors.New("kill denied")
	err = abortRuntimeSetupFailureWithAbort(
		nil,
		nil,
		st,
		id,
		pid,
		start,
		setupCause,
		func(_ *exec.Cmd, _ *os.File) (bool, error) { return false, abortCause },
	)
	if err == nil {
		t.Fatal("unreaped setup abort unexpectedly succeeded")
	}
	if !errors.Is(err, setupCause) || !errors.Is(err, abortCause) {
		t.Fatalf("error=%v, want setup and abort causes", err)
	}
	if !isRuntimeControlError(err) {
		t.Fatalf("unreaped setup abort not classified runtime-control: %v", err)
	}
	if !strings.Contains(err.Error(), "not confirmed reaped") {
		t.Fatalf("error=%v, want explicit reap-proof failure", err)
	}

	current, err := st.Get(id)
	if err != nil {
		t.Fatal(err)
	}
	if current.Status != state.StatusRunning {
		t.Fatalf("status=%s, want running while reap is unproven", current.Status)
	}
	if current.PID != pid || current.PIDStartTime != start {
		t.Fatalf("running identity changed: pid=%d start=%d, want %d/%d", current.PID, current.PIDStartTime, pid, start)
	}
	if current.FinishedAt != nil {
		t.Fatalf("running generation gained FinishedAt=%v without reap proof", current.FinishedAt)
	}
}
