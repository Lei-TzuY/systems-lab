package container

import (
	"errors"
	"fmt"
	"time"

	"minicontainer/internal/state"
)

type preGenerationRunError struct {
	err error
}

func (e *preGenerationRunError) Error() string { return e.err.Error() }
func (e *preGenerationRunError) Unwrap() error { return e.err }

func markPreGenerationRunFailure(err error) error {
	if err == nil {
		return nil
	}
	return &preGenerationRunError{err: err}
}

func isPreGenerationRunFailure(err error) bool {
	var preGeneration *preGenerationRunError
	return errors.As(err, &preGeneration)
}

// finalizeCreatedRunFailure records only failures that are proven to have
// happened before cmd.Start admitted any process. Once a child has been
// spawned, a still-created record is not enough proof that no live process
// exists, so this helper must not synthesize stopped state for unmarked errors.
func finalizeCreatedRunFailure(st *state.Store, id string, runErr error, finishedAt time.Time) error {
	if st == nil || id == "" || runErr == nil || !isPreGenerationRunFailure(runErr) {
		return runErr
	}

	changed, err := st.MarkStoppedIfCreated(id, 1, finishedAt)
	if err != nil {
		return errors.Join(
			runErr,
			&runtimeStateError{err: fmt.Errorf("persist pre-generation stopped state for container %s: %w", id, err)},
		)
	}
	if !changed {
		return runErr
	}
	return runErr
}
