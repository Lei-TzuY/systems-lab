package container

import (
	"errors"
	"fmt"
	"time"

	"minicontainer/internal/state"
)

const waitStateRefresh = 250 * time.Millisecond

// WaitContainer waits for the lifecycle represented by containerID to reach a
// stopped state and returns its persisted exit code. For a running record it
// binds to the exact PID/start-time identity with a pidfd instead of polling a
// numeric PID. If the persisted identity is already gone or has been reused,
// the stale running record is reconciled to stopped. Durable host-side cleanup
// tokens are retried before a stopped result is returned.
func WaitContainer(st *state.Store, containerID string) (int, error) {
	if st == nil {
		return -1, fmt.Errorf("state store is nil")
	}

	for {
		c, err := st.Resolve(containerID)
		if err != nil {
			return -1, fmt.Errorf("resolve container: %w", err)
		}
		if c.Status == state.StatusStopped {
			if err := CleanupStoppedRuntimeResources(st, c); err != nil {
				return c.ExitCode, fmt.Errorf("cleanup pending runtime resources for stopped container %s: %w", c.ID, err)
			}
			return c.ExitCode, nil
		}
		if c.Status != state.StatusRunning {
			return -1, fmt.Errorf("container %s is %s; wait requires running or stopped state", c.ID, c.Status)
		}
		if c.PID <= 0 || c.PIDStartTime == 0 {
			return -1, fmt.Errorf("container %s: %w", c.ID, ErrProcessIdentityUnavailable)
		}

		handle, err := OpenProcessHandle(c.PID, c.PIDStartTime)
		if err != nil {
			if errors.Is(err, ErrProcessNotFound) || errors.Is(err, ErrProcessIdentityMismatch) {
				changed, finalizeErr := FinalizeStoppedGeneration(st, c, -1, time.Now())
				if finalizeErr != nil {
					return -1, fmt.Errorf("finalize stale running state for container %s: %w", c.ID, finalizeErr)
				}
				if changed {
					return -1, nil
				}
				continue
			}
			return -1, fmt.Errorf("open process handle for container %s: %w", c.ID, err)
		}

		for {
			exited, waitErr := handle.WaitExit(waitStateRefresh)
			if waitErr != nil {
				_ = handle.Close()
				return -1, fmt.Errorf("wait for container %s process: %w", c.ID, waitErr)
			}

			latest, stateErr := st.Get(c.ID)
			if stateErr != nil {
				_ = handle.Close()
				return -1, fmt.Errorf("reload container %s state while waiting: %w", c.ID, stateErr)
			}
			if latest.Status == state.StatusStopped {
				_ = handle.Close()
				if exited {
					if _, finalizeErr := FinalizeStoppedGeneration(st, c, latest.ExitCode, time.Now()); finalizeErr != nil {
						return latest.ExitCode, fmt.Errorf("cleanup stopped container %s generation: %w", c.ID, finalizeErr)
					}
				}
				refreshed, err := st.Get(c.ID)
				if err != nil {
					return latest.ExitCode, fmt.Errorf("reload stopped container %s before cleanup: %w", c.ID, err)
				}
				if err := CleanupStoppedRuntimeResources(st, refreshed); err != nil {
					return refreshed.ExitCode, fmt.Errorf("cleanup pending runtime resources for stopped container %s: %w", c.ID, err)
				}
				return refreshed.ExitCode, nil
			}
			if latest.Status != state.StatusRunning || latest.PID != c.PID || latest.PIDStartTime != c.PIDStartTime {
				_ = handle.Close()
				break
			}
			if !exited {
				continue
			}

			_ = handle.Close()
			changed, finalizeErr := FinalizeStoppedGeneration(st, c, -1, time.Now())
			if finalizeErr != nil {
				return -1, fmt.Errorf("finalize exited container %s: %w", c.ID, finalizeErr)
			}
			if changed {
				return -1, nil
			}
			break
		}
	}
}
