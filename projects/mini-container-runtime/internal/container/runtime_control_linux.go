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

// runtimeControlError marks failures owned by the runtime rather than the
// container payload. Restart policies must never retry these errors because a
// retry cannot fix missing isolation/resource controls and may create repeated
// unmanaged or incorrectly constrained processes.
type runtimeControlError interface {
	error
	runtimeControlFailure()
}

func (*runtimeStateError) runtimeControlFailure() {}

type runtimeSetupError struct {
	err error
}

func (e *runtimeSetupError) Error() string { return e.err.Error() }
func (e *runtimeSetupError) Unwrap() error { return e.err }
func (e *runtimeSetupError) runtimeControlFailure() {}

func isRuntimeControlError(err error) bool {
	if err == nil {
		return false
	}
	var controlErr runtimeControlError
	return errors.As(err, &controlErr)
}

func resourceLimitsRequested(cfg Config) bool {
	return cfg.Memory != 0 || cfg.CPUWeight != 0 || cfg.CPUs != 0 || cfg.PidsLimit != 0
}

func runtimeCgroupName(containerID string, childPID int, childStartTime uint64, managed bool) (string, error) {
	if !managed {
		if childPID <= 0 {
			return "", fmt.Errorf("invalid cgroup target PID %d", childPID)
		}
		return fmt.Sprintf("minicontainer-%d", childPID), nil
	}
	return cgroups.NameForContainerProcess(containerID, childPID, childStartTime)
}

// abortRuntimeSetupFailure terminates and reaps a child that is still blocked
// on the parent/child sync pipe, reconciles managed lifecycle state, then cleans
// only cgroup paths that the child was observed to own and durable cleanup
// tokens belonging to that exact process generation. Capturing cgroup membership
// before termination avoids deleting a same-named cgroup when Apply failed
// because the runtime never acquired it. For managed containers, destructive
// cleanup never precedes a durable stopped transition.
func abortRuntimeSetupFailure(
	cmd *exec.Cmd,
	writePipe *os.File,
	lifecycleStore *state.Store,
	containerID string,
	childPID int,
	childStartTime uint64,
	cause error,
) error {
	return abortRuntimeSetupFailureWithAbort(
		cmd,
		writePipe,
		lifecycleStore,
		containerID,
		childPID,
		childStartTime,
		cause,
		abortBlockedChildChecked,
	)
}

func abortRuntimeSetupFailureWithAbort(
	cmd *exec.Cmd,
	writePipe *os.File,
	lifecycleStore *state.Store,
	containerID string,
	childPID int,
	childStartTime uint64,
	cause error,
	abort blockedChildAborter,
) error {
	setupErr := &runtimeSetupError{err: cause}
	resultErr := error(setupErr)

	var ownedCgroups *cgroups.ProcessCleanup
	cgroupName, nameErr := runtimeCgroupName(containerID, childPID, childStartTime, lifecycleStore != nil)
	if nameErr != nil {
		resultErr = errors.Join(resultErr, fmt.Errorf("derive aborted cgroup identity: %w", nameErr))
	} else {
		captured, captureErr := cgroups.CaptureProcessCleanup(cgroupName, childPID)
		if captureErr != nil {
			resultErr = errors.Join(resultErr, fmt.Errorf("capture aborted cgroup ownership: %w", captureErr))
		} else {
			ownedCgroups = captured
		}
	}

	if abort == nil {
		return errors.Join(resultErr, &runtimeSetupError{err: fmt.Errorf("blocked child abort operation is nil; preserving running lifecycle and resource ownership")})
	}
	reaped, abortErr := abort(cmd, writePipe)
	if abortErr != nil {
		resultErr = errors.Join(resultErr, &runtimeSetupError{err: fmt.Errorf("abort blocked child: %w", abortErr)})
	}
	if !reaped {
		return errors.Join(resultErr, &runtimeSetupError{err: fmt.Errorf("blocked child was not confirmed reaped; preserving running lifecycle and resource ownership")})
	}

	if lifecycleStore == nil {
		if ownedCgroups != nil && !ownedCgroups.Empty() {
			if err := ownedCgroups.Remove(false); err != nil {
				resultErr = errors.Join(resultErr, fmt.Errorf("cleanup aborted cgroup: %w", err))
			}
		}
		return resultErr
	}

	changed, stateErr := lifecycleStore.MarkStoppedIfIdentity(
		containerID,
		childPID,
		childStartTime,
		-1,
		time.Now(),
	)
	if stateErr != nil {
		resultErr = errors.Join(
			resultErr,
			&runtimeStateError{err: fmt.Errorf("persist stopped state after runtime setup failure for container %s: %w", containerID, stateErr)},
		)
		if !changed {
			// The child is gone, but the durable lifecycle record still does not
			// prove that. Preserve captured cgroups and all ownership sidecars so
			// reconciliation retains a complete retry proof.
			return resultErr
		}
	}

	if ownedCgroups != nil && !ownedCgroups.Empty() {
		if err := ownedCgroups.Remove(false); err != nil {
			resultErr = errors.Join(resultErr, fmt.Errorf("cleanup aborted cgroup: %w", err))
		}
	}

	current, readErr := lifecycleStore.Get(containerID)
	if readErr != nil {
		resultErr = errors.Join(resultErr, &runtimeStateError{err: fmt.Errorf("reload container after runtime setup failure for container %s: %w", containerID, readErr)})
		return resultErr
	}
	if current.Status == state.StatusStopped {
		if cleanupErr := cleanupRuntimeGenerationResources(lifecycleStore, containerID, childPID, childStartTime); cleanupErr != nil {
			resultErr = errors.Join(resultErr, &runtimeSetupError{err: fmt.Errorf("cleanup persisted runtime resources after runtime setup failure for container %s: %w", containerID, cleanupErr)})
		}
	}
	return resultErr
}
