//go:build linux

package container

import (
	"errors"
	"fmt"
	"os"
	"os/exec"
	"time"

	"minicontainer/internal/cgroups"
	"minicontainer/internal/state"
)

type appliedCgroupCleanupFunc func(name string, debug bool) error

// persistAppliedCgroupOwnership records the cleanup token for a managed cgroup
// before the blocked child is released. Apply has already returned success, so
// this parent has authoritative proof that it created and owns cgroupName.
// Failure to persist that proof is a runtime-control failure: the child is
// reaped and the known-owned cgroup is removed only after stopped lifecycle
// state is durably committed.
func persistAppliedCgroupOwnership(
	cmd *exec.Cmd,
	writePipe *os.File,
	st *state.Store,
	containerID string,
	pid int,
	pidStartTime uint64,
	cgroupName string,
	debug bool,
) error {
	return persistAppliedCgroupOwnershipWithAbort(
		cmd,
		writePipe,
		st,
		containerID,
		pid,
		pidStartTime,
		cgroupName,
		debug,
		abortBlockedChildChecked,
	)
}

func persistAppliedCgroupOwnershipWithAbort(
	cmd *exec.Cmd,
	writePipe *os.File,
	st *state.Store,
	containerID string,
	pid int,
	pidStartTime uint64,
	cgroupName string,
	debug bool,
	abort blockedChildAborter,
) error {
	return persistAppliedCgroupOwnershipWithAbortAndCleanup(
		cmd,
		writePipe,
		st,
		containerID,
		pid,
		pidStartTime,
		cgroupName,
		debug,
		abort,
		cgroups.RemoveChecked,
	)
}

func persistAppliedCgroupOwnershipWithAbortAndCleanup(
	cmd *exec.Cmd,
	writePipe *os.File,
	st *state.Store,
	containerID string,
	pid int,
	pidStartTime uint64,
	cgroupName string,
	debug bool,
	abort blockedChildAborter,
	cleanup appliedCgroupCleanupFunc,
) error {
	if st == nil {
		return nil
	}
	if err := st.MarkCgroupOwnedIfIdentity(containerID, pid, pidStartTime, cgroupName); err == nil {
		return nil
	} else {
		persistErr := err
		resultErr := error(&runtimeStateError{err: fmt.Errorf("persist cgroup ownership for container %s: %w", containerID, persistErr)})

		if abort == nil {
			return errors.Join(resultErr, &runtimeSetupError{err: fmt.Errorf("blocked child abort operation is nil; preserving running lifecycle and owned cgroup %s", cgroupName)})
		}
		reaped, abortErr := abort(cmd, writePipe)
		if abortErr != nil {
			resultErr = errors.Join(resultErr, &runtimeSetupError{err: fmt.Errorf("abort blocked child after cgroup ownership persistence failure: %w", abortErr)})
		}
		if !reaped {
			return errors.Join(resultErr, &runtimeSetupError{err: fmt.Errorf("blocked child was not confirmed reaped; preserving running lifecycle and owned cgroup %s", cgroupName)})
		}

		changed, stateErr := st.MarkStoppedIfIdentity(containerID, pid, pidStartTime, -1, time.Now())
		if stateErr != nil {
			resultErr = errors.Join(resultErr, &runtimeStateError{err: fmt.Errorf("persist stopped state after cgroup ownership failure for container %s: %w", containerID, stateErr)})
			if !changed {
				// The cgroup is known-owned, but without a durable stopped record
				// deleting it would destroy host-side evidence while state still
				// claims (or cannot disprove) that this generation is running.
				return resultErr
			}
		}

		if cleanup == nil {
			return errors.Join(resultErr, &runtimeSetupError{err: fmt.Errorf("owned cgroup cleanup operation is nil; preserving cgroup %s", cgroupName)})
		}
		if cleanupErr := cleanup(cgroupName, debug); cleanupErr != nil {
			return errors.Join(resultErr, &runtimeSetupError{err: fmt.Errorf("cleanup owned cgroup %s after ownership persistence failure: %w", cgroupName, cleanupErr)})
		}

		_, clearErr := st.ClearCgroupOwnershipIfMatch(containerID, state.CgroupOwnership{
			Name:         cgroupName,
			PID:          pid,
			PIDStartTime: pidStartTime,
		})
		if clearErr != nil {
			resultErr = errors.Join(resultErr, &runtimeStateError{err: fmt.Errorf("clear cgroup ownership after cleanup for container %s: %w", containerID, clearErr)})
		}
		return resultErr
	}
}
