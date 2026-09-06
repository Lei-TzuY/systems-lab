package container

import (
	"errors"
	"fmt"
	"time"

	"minicontainer/internal/state"
)

type processGenerationProbe func(pid int, pidStartTime uint64) (alive bool, err error)
type stoppedGenerationCleanup func(st *state.Store, c *state.Container) error
type stoppedGenerationFinalizer func(st *state.Store, c *state.Container, exitCode int, finishedAt time.Time) (changed bool, err error)

// ReconcileContainerState refreshes one persisted container lifecycle using its
// exact process generation. Running state is never inferred from a numeric PID
// alone: a pidfd is opened and verified against the persisted /proc starttime.
//
// A missing process or a reused PID proves that the persisted generation is
// gone. Recovery delegates the transition and all generation-scoped host
// cleanup to FinalizeStoppedGeneration so a durable stop commit followed by a
// housekeeping error cannot strand cleanup until some unrelated later command.
// Stopped records also retry every durable runtime cleanup token before callers
// such as rm/prune are allowed to discard state.
//
// Once a non-nil snapshot is supplied, errors preserve at least that snapshot
// (or a newer one already read from disk) so callers can report failures without
// dereferencing a record that disappeared during reconciliation.
func ReconcileContainerState(st *state.Store, c *state.Container) (*state.Container, error) {
	return reconcileContainerStateWithFinalizer(st, c, probeProcessGeneration, CleanupStoppedRuntimeResources, FinalizeStoppedGeneration, time.Now)
}

func probeProcessGeneration(pid int, pidStartTime uint64) (bool, error) {
	handle, err := OpenProcessHandle(pid, pidStartTime)
	if err != nil {
		if errors.Is(err, ErrProcessNotFound) || errors.Is(err, ErrProcessIdentityMismatch) {
			return false, nil
		}
		return false, err
	}

	exited, waitErr := handle.WaitExit(0)
	closeErr := handle.Close()
	if waitErr != nil {
		return false, waitErr
	}
	if closeErr != nil {
		return false, closeErr
	}
	return !exited, nil
}

// reconcileContainerStateWith keeps the historical test seam for stopped-state
// cleanup while exercising the same ordering as the production finalizer.
func reconcileContainerStateWith(
	st *state.Store,
	c *state.Container,
	probe processGenerationProbe,
	cleanup stoppedGenerationCleanup,
	now func() time.Time,
) (*state.Container, error) {
	finalize := func(st *state.Store, c *state.Container, exitCode int, finishedAt time.Time) (bool, error) {
		changed, err := st.MarkStoppedIfIdentity(c.ID, c.PID, c.PIDStartTime, exitCode, finishedAt)
		if err != nil {
			return changed, err
		}
		latest, err := st.Get(c.ID)
		if err != nil {
			return changed, err
		}
		if latest.Status == state.StatusStopped {
			if err := cleanup(st, latest); err != nil {
				return changed, err
			}
		}
		return changed, nil
	}
	return reconcileContainerStateWithFinalizer(st, c, probe, cleanup, finalize, now)
}

func reconcileContainerStateWithFinalizer(
	st *state.Store,
	c *state.Container,
	probe processGenerationProbe,
	cleanup stoppedGenerationCleanup,
	finalize stoppedGenerationFinalizer,
	now func() time.Time,
) (*state.Container, error) {
	if c == nil {
		return nil, fmt.Errorf("container snapshot is nil")
	}
	if st == nil {
		return c, fmt.Errorf("state store is nil")
	}
	if c.ID == "" {
		return c, fmt.Errorf("container ID is empty")
	}
	if probe == nil {
		return c, fmt.Errorf("process generation probe is nil")
	}
	if cleanup == nil {
		return c, fmt.Errorf("stopped generation cleanup is nil")
	}
	if finalize == nil {
		return c, fmt.Errorf("stopped generation finalizer is nil")
	}
	if now == nil {
		return c, fmt.Errorf("clock is nil")
	}

	current, err := st.Get(c.ID)
	if err != nil {
		return c, fmt.Errorf("reload container %s before reconciliation: %w", c.ID, err)
	}

	if current.Status == state.StatusStopped {
		if err := cleanup(st, current); err != nil {
			return current, fmt.Errorf("cleanup stopped container %s during reconciliation: %w", current.ID, err)
		}
		latest, err := st.Get(current.ID)
		if err != nil {
			return current, fmt.Errorf("reload stopped container %s after cleanup: %w", current.ID, err)
		}
		return latest, nil
	}
	if current.Status != state.StatusRunning {
		return current, nil
	}
	if current.PID <= 0 || current.PIDStartTime == 0 {
		return current, fmt.Errorf("%w: container %s has invalid process identity %d/%d", ErrProcessIdentityUnavailable, current.ID, current.PID, current.PIDStartTime)
	}

	pid := current.PID
	pidStartTime := current.PIDStartTime
	alive, err := probe(pid, pidStartTime)
	if err != nil {
		return current, fmt.Errorf("probe container %s process %d/%d: %w", current.ID, pid, pidStartTime, err)
	}
	if alive {
		latest, err := st.Get(current.ID)
		if err != nil {
			return current, fmt.Errorf("reload live container %s after process probe: %w", current.ID, err)
		}
		return latest, nil
	}

	changed, finalizeErr := finalize(st, current, -1, now())
	latest, reloadErr := st.Get(current.ID)
	if reloadErr != nil {
		if finalizeErr != nil {
			return current, errors.Join(
				fmt.Errorf("finalize exited container %s generation %d/%d: %w", current.ID, pid, pidStartTime, finalizeErr),
				fmt.Errorf("reload container %s after generation finalization: %w", current.ID, reloadErr),
			)
		}
		return current, fmt.Errorf("reload container %s after generation finalization: %w", current.ID, reloadErr)
	}
	if finalizeErr != nil {
		// The finalizer may report a post-commit cleanup/housekeeping failure.
		// Return the durable latest snapshot rather than the stale pre-stop one;
		// durable ownership tokens remain available for a subsequent retry.
		return latest, fmt.Errorf("finalize exited container %s generation %d/%d (changed=%t): %w", current.ID, pid, pidStartTime, changed, finalizeErr)
	}
	return latest, nil
}
