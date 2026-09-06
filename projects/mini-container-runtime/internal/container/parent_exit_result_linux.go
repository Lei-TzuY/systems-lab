//go:build linux

package container

import (
	"errors"
	"fmt"
	"os/exec"
	"time"

	"minicontainer/internal/state"
)

type managedGenerationFinalizer func(*state.Store, *state.Container, int, time.Time) (bool, error)

// finalizeManagedParentExit reconciles the authoritative parent-side lifecycle
// state after the child has exited. Production callers provide the generation
// finalizer unconditionally so network/DNS ownership is cleaned only after the
// durable stopped transition even when best-effort cgroup Apply did not succeed.
// The nil-finalizer fallback is retained only for legacy focused tests/callers
// that explicitly have no generation-owned resources to reconcile.
func finalizeManagedParentExit(
	st *state.Store,
	snapshot *state.Container,
	exitCode int,
	finishedAt time.Time,
	cgroupApplied bool,
	finalizeGeneration managedGenerationFinalizer,
) error {
	if st == nil {
		return fmt.Errorf("state store is nil")
	}
	if snapshot == nil {
		return fmt.Errorf("container snapshot is nil")
	}

	if finalizeGeneration != nil {
		_, err := finalizeGeneration(st, snapshot, exitCode, finishedAt)
		return err
	}
	if cgroupApplied {
		return fmt.Errorf("generation finalizer is nil")
	}

	_, err := st.MarkStoppedIfIdentity(
		snapshot.ID,
		snapshot.PID,
		snapshot.PIDStartTime,
		exitCode,
		finishedAt,
	)
	if err != nil {
		return fmt.Errorf("persist stopped state for container %s: %w", snapshot.ID, err)
	}
	return nil
}

// cleanupBridgeAfterNormalExit runs an eager bridge teardown only for unmanaged
// callers. Managed runtime generations persist exact network ownership and must
// let FinalizeStoppedGeneration perform teardown after the stopped transition
// is durable; running this closure first would make networking disappear while
// DNS and lifecycle state still advertise a live generation.
func cleanupBridgeAfterNormalExit(st *state.Store, bridgeCleanup func() error) error {
	if bridgeCleanup == nil || st != nil {
		return nil
	}
	if err := bridgeCleanup(); err != nil {
		return &runtimeSetupError{err: fmt.Errorf("cleanup bridge network: %w", err)}
	}
	return nil
}

// parentExitResult combines the payload result with authoritative parent-side
// teardown failures. Teardown failures are runtime-control failures: restart
// policies must not launch another generation while isolation cleanup or
// lifecycle finalization is incomplete. The original *exec.ExitError remains
// discoverable through errors.As when the payload itself exited non-zero.
func parentExitResult(waitErr, finalizationErr, bridgeCleanupErr error) error {
	var resultErr error
	if waitErr != nil {
		if exitErr, ok := waitErr.(*exec.ExitError); ok {
			resultErr = exitErr
		} else {
			resultErr = fmt.Errorf("container exited with error: %w", waitErr)
		}
	}
	if finalizationErr != nil {
		resultErr = errors.Join(resultErr, &runtimeSetupError{err: fmt.Errorf("finalize stopped process generation: %w", finalizationErr)})
	}
	if bridgeCleanupErr != nil {
		resultErr = errors.Join(resultErr, bridgeCleanupErr)
	}
	return resultErr
}
