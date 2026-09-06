package container

import (
	"errors"
	"testing"
	"time"

	"minicontainer/internal/state"
)

func saveCreatedRunFailureContainer(t *testing.T, st *state.Store, id string) {
	t.Helper()
	if err := st.Save(&state.Container{ID: id, Status: state.StatusCreated}); err != nil {
		t.Fatalf("save container: %v", err)
	}
}

func TestFinalizeCreatedRunFailureCommitsSyntheticStop(t *testing.T) {
	st, err := state.Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	const id = "pre-generation"
	saveCreatedRunFailureContainer(t, st, id)
	finishedAt := time.Unix(100, 0)
	cause := errors.New("spawn admission failed")

	gotErr := finalizeCreatedRunFailure(st, id, markPreGenerationRunFailure(cause), finishedAt)
	if !errors.Is(gotErr, cause) {
		t.Fatalf("error=%v, want original cause", gotErr)
	}
	got, err := st.Get(id)
	if err != nil {
		t.Fatal(err)
	}
	if got.Status != state.StatusStopped || got.ExitCode != 1 {
		t.Fatalf("state=%+v, want synthetic stopped/1", got)
	}
	if got.StartedAt != nil || got.FinishedAt == nil || !got.FinishedAt.Equal(finishedAt) {
		t.Fatalf("timestamps=%+v", got)
	}
}

func TestFinalizeCreatedRunFailureRejectsUnprovenPostSpawnError(t *testing.T) {
	st, err := state.Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	const id = "possibly-spawned"
	saveCreatedRunFailureContainer(t, st, id)
	cause := errors.New("identity capture failed after spawn")

	gotErr := finalizeCreatedRunFailure(st, id, cause, time.Unix(105, 0))
	if !errors.Is(gotErr, cause) {
		t.Fatalf("error=%v, want original cause", gotErr)
	}
	got, err := st.Get(id)
	if err != nil {
		t.Fatal(err)
	}
	if got.Status != state.StatusCreated || got.FinishedAt != nil {
		t.Fatalf("unproven failure mutated lifecycle state: %+v", got)
	}
}

func TestFinalizeCreatedRunFailureDoesNotOverwriteGenerationState(t *testing.T) {
	st, err := state.Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	const id = "generation-owned"
	saveCreatedRunFailureContainer(t, st, id)
	startedAt := time.Unix(110, 0)
	if err := st.MarkRunning(id, 4242, 77, startedAt); err != nil {
		t.Fatal(err)
	}
	cause := errors.New("runtime returned")

	gotErr := finalizeCreatedRunFailure(st, id, markPreGenerationRunFailure(cause), time.Unix(111, 0))
	if !errors.Is(gotErr, cause) {
		t.Fatalf("error=%v, want original cause", gotErr)
	}
	got, err := st.Get(id)
	if err != nil {
		t.Fatal(err)
	}
	if got.Status != state.StatusRunning || got.PID != 4242 || got.PIDStartTime != 77 {
		t.Fatalf("generation state mutated: %+v", got)
	}
	if got.FinishedAt != nil {
		t.Fatalf("finished_at=%v, want nil", got.FinishedAt)
	}
}

func TestFinalizeCreatedRunFailureIgnoresSuccessfulReturn(t *testing.T) {
	st, err := state.Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	const id = "success"
	saveCreatedRunFailureContainer(t, st, id)

	if err := finalizeCreatedRunFailure(st, id, nil, time.Unix(120, 0)); err != nil {
		t.Fatalf("error=%v", err)
	}
	got, err := st.Get(id)
	if err != nil {
		t.Fatal(err)
	}
	if got.Status != state.StatusCreated || got.FinishedAt != nil {
		t.Fatalf("successful return mutated state: %+v", got)
	}
}
