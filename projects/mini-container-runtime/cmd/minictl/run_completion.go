package main

import (
	"errors"
	"fmt"
	"time"

	"minicontainer/internal/state"
)

// settleRunCommandState reconciles the synchronous `minictl run` return with
// lifecycle state owned entirely by container.Run. The CLI only reloads and
// validates the authoritative record; it never synthesizes lifecycle state.
func settleRunCommandState(st *state.Store, id string, runErr error, finishedAt time.Time) (*state.Container, error) {
	_ = finishedAt // retained for call-site compatibility; runtime owns timestamps.
	if st == nil {
		return nil, fmt.Errorf("state store is nil")
	}
	if id == "" {
		return nil, fmt.Errorf("container ID is empty")
	}

	current, err := st.Get(id)
	if err != nil {
		return nil, fmt.Errorf("reload container %s after run: %w", id, err)
	}

	switch current.Status {
	case state.StatusStopped:
		return current, nil

	case state.StatusCreated:
		if runErr == nil {
			return current, fmt.Errorf("runtime returned successfully but container %s never left created state", id)
		}
		return current, fmt.Errorf(
			"runtime returned with error while container %s remains created; pre-generation failure was not durably finalized: %w",
			id,
			runErr,
		)

	case state.StatusRunning:
		return current, fmt.Errorf(
			"runtime returned while container %s remains running as process %d/%d",
			id,
			current.PID,
			current.PIDStartTime,
		)

	default:
		return current, fmt.Errorf("container %s has unknown lifecycle status %q after run", id, current.Status)
	}
}

func joinRunCommandErrors(runErr, stateErr error) error {
	return errors.Join(runErr, stateErr)
}
