package container

import (
	"errors"
	"fmt"
	"time"

	"minicontainer/internal/cgroups"
	"minicontainer/internal/dns"
	"minicontainer/internal/events"
	"minicontainer/internal/state"
)

type generationCleanupFunc func(containerID string, pid int, pidStartTime uint64) error

// FinalizeStoppedGeneration reconciles lifecycle state and cleans every durable
// host-side resource owned by one exact container process generation. Callers
// must invoke it only after they have established that the referenced
// PID/start-time process has exited (or that the PID now belongs to another
// generation).
func FinalizeStoppedGeneration(st *state.Store, c *state.Container, exitCode int, finishedAt time.Time) (bool, error) {
	changed, cgroupErr := finalizeStoppedGenerationWithCleanup(st, c, exitCode, finishedAt, cleanupContainerProcessGeneration)
	if st == nil || c == nil || c.ID == "" {
		return changed, cgroupErr
	}

	current, readErr := st.Get(c.ID)
	if readErr != nil {
		return changed, errors.Join(cgroupErr, fmt.Errorf("reload container after generation finalization: %w", readErr))
	}
	if current.Status != state.StatusStopped {
		return changed, cgroupErr
	}

	// A delayed finalizer can observe a later generation that has independently
	// restarted and stopped. Bind destructive external cleanup to the durable
	// stopped revision's exited PID/start-time identity. This is stronger than
	// status alone and turns a stale A-generation actor into a no-op after B has
	// become the current stopped state.
	exitedPID, exitedStartTime, revisionCurrent, identityOK, identityErr := st.GetExitedIdentityForStoppedRevision(current.ID, current.Revision)
	if identityErr != nil {
		return changed, errors.Join(cgroupErr, fmt.Errorf("validate stopped generation identity before external cleanup: %w", identityErr))
	}
	if !revisionCurrent || !identityOK || exitedPID != c.PID || exitedStartTime != c.PIDStartTime {
		return changed, cgroupErr
	}

	// Die is generation-scoped, just like MarkStoppedIfIdentity. Emit it only
	// for the actor that actually transitioned this exact PID/start-time record;
	// retries/reconcilers observing an already-stopped record cannot duplicate it.
	// Event persistence is best effort and must not revoke an already-finished
	// payload generation.
	if changed {
		_ = events.Publish(
			events.EventDie,
			c.ID,
			current.RootFS,
			fmt.Sprintf("exited with code %d", exitCode),
		)
	}

	externalErr := cleanupStoppedGenerationExternalResourcesWith(
		st,
		c.ID,
		c.PID,
		c.PIDStartTime,
		cleanupNetworkGenerationIfOwned,
		dns.CleanupStoppedHostRegistrationGeneration,
	)
	return changed, errors.Join(cgroupErr, externalErr)
}

func cleanupStoppedGenerationExternalResourcesWith(
	st *state.Store,
	containerID string,
	pid int,
	pidStartTime uint64,
	networkCleanup func(*state.Store, string, int, uint64, bool) error,
	dnsCleanup func(string, string, int, uint64) error,
) error {
	var dnsErr error
	if dnsCleanup == nil {
		dnsErr = fmt.Errorf("DNS generation cleanup function is nil")
	} else if err := dnsCleanup(defaultBridgeDNSNetwork, containerID, pid, pidStartTime); err != nil {
		dnsErr = fmt.Errorf("cleanup bridge DNS registration for stopped container %s: %w", containerID, err)
	}

	var networkErr error
	if networkCleanup == nil {
		networkErr = fmt.Errorf("network generation cleanup function is nil")
	} else if err := networkCleanup(st, containerID, pid, pidStartTime, false); err != nil {
		networkErr = err
	}
	return errors.Join(dnsErr, networkErr)
}

func validateOwnedGenerationName(containerID string, ownership state.CgroupOwnership) error {
	expected, err := cgroups.NameForContainerProcess(containerID, ownership.PID, ownership.PIDStartTime)
	if err != nil {
		return fmt.Errorf("derive expected owned cgroup name: %w", err)
	}
	if ownership.Name != expected {
		return fmt.Errorf("persisted cgroup ownership name %q does not match expected generation name %q", ownership.Name, expected)
	}
	return nil
}

func clearOwnedGenerationAfterCleanup(st *state.Store, containerID string, ownership state.CgroupOwnership) error {
	cleared, err := st.ClearCgroupOwnershipIfMatch(containerID, ownership)
	if err != nil {
		return err
	}
	if cleared {
		return nil
	}

	current, ok, err := st.GetCgroupOwnership(containerID)
	if err != nil {
		return err
	}
	if !ok {
		// Another lifecycle actor may have completed the same cleanup first.
		return nil
	}
	if current != ownership {
		return fmt.Errorf("cgroup ownership changed while clearing cleaned generation: now %s (%d/%d)", current.Name, current.PID, current.PIDStartTime)
	}
	return fmt.Errorf("cgroup ownership remained after successful cleanup")
}

func cleanupOwnedGenerationWith(
	st *state.Store,
	containerID string,
	ownership state.CgroupOwnership,
	cleanup generationCleanupFunc,
) error {
	if cleanup == nil {
		return fmt.Errorf("generation cleanup function is nil")
	}
	if err := validateOwnedGenerationName(containerID, ownership); err != nil {
		return err
	}
	if err := cleanup(containerID, ownership.PID, ownership.PIDStartTime); err != nil {
		return err
	}
	if err := clearOwnedGenerationAfterCleanup(st, containerID, ownership); err != nil {
		return fmt.Errorf("clear cgroup ownership after cleanup: %w", err)
	}
	return nil
}

// CleanupStoppedCgroup retries cleanup for a stopped container whose durable
// ownership sidecar survived an earlier cleanup failure. Legacy/unowned stopped
// containers have no sidecar and are a no-op.
func CleanupStoppedCgroup(st *state.Store, c *state.Container) error {
	return cleanupStoppedCgroupWithCleanup(st, c, cleanupContainerProcessGeneration)
}

func cleanupStoppedCgroupWithCleanup(st *state.Store, c *state.Container, cleanup generationCleanupFunc) error {
	if st == nil {
		return fmt.Errorf("state store is nil")
	}
	if c == nil {
		return fmt.Errorf("container snapshot is nil")
	}
	if c.ID == "" {
		return fmt.Errorf("container ID is empty")
	}
	if c.Status != state.StatusStopped {
		return fmt.Errorf("container %s is %s; cgroup cleanup retry requires stopped state", c.ID, c.Status)
	}
	current, err := stoppedSnapshotStillCurrent(st, c)
	if err != nil {
		return fmt.Errorf("validate stopped cleanup snapshot for container %s: %w", c.ID, err)
	}
	if !current {
		return nil
	}

	ownership, ok, err := st.GetCgroupOwnership(c.ID)
	if err != nil {
		return fmt.Errorf("read cgroup ownership for stopped container %s: %w", c.ID, err)
	}
	if !ok {
		return nil
	}

	exitedPID, exitedStartTime, revisionCurrent, identityOK, identityRequired, err := st.GetStoppedExitIdentityPolicy(c.ID, c.Revision)
	if err != nil {
		return fmt.Errorf("read stopped generation identity before cgroup cleanup for container %s: %w", c.ID, err)
	}
	if !revisionCurrent {
		return nil
	}
	if !identityOK {
		if identityRequired {
			return fmt.Errorf("stopped container %s revision %d is missing required exited process identity while cgroup ownership remains", c.ID, c.Revision)
		}
		return fmt.Errorf("refusing cgroup cleanup for stopped container %s revision %d without exact exited process identity", c.ID, c.Revision)
	}
	if ownership.PID != exitedPID || ownership.PIDStartTime != exitedStartTime {
		return fmt.Errorf(
			"refusing cgroup cleanup for container %s: ownership belongs to process %d/%d, stopped generation is %d/%d",
			c.ID,
			ownership.PID,
			ownership.PIDStartTime,
			exitedPID,
			exitedStartTime,
		)
	}
	if err := cleanupOwnedGenerationWith(st, c.ID, ownership, cleanup); err != nil {
		return fmt.Errorf("cleanup persisted cgroup for stopped container %s: %w", c.ID, err)
	}
	return nil
}

func finalizeStoppedGenerationWithCleanup(
	st *state.Store,
	c *state.Container,
	exitCode int,
	finishedAt time.Time,
	cleanup generationCleanupFunc,
) (bool, error) {
	if st == nil {
		return false, fmt.Errorf("state store is nil")
	}
	if c == nil {
		return false, fmt.Errorf("container snapshot is nil")
	}
	if c.ID == "" || c.PID <= 0 || c.PIDStartTime == 0 {
		return false, fmt.Errorf("container process generation is incomplete")
	}
	if cleanup == nil {
		return false, fmt.Errorf("generation cleanup function is nil")
	}

	changed, stateErr := st.MarkStoppedIfIdentity(c.ID, c.PID, c.PIDStartTime, exitCode, finishedAt)
	if stateErr != nil {
		stateErr = fmt.Errorf("persist stopped state for container %s: %w", c.ID, stateErr)
		if !changed {
			// Destructive host cleanup must never run before stopped state is
			// durable. MarkStoppedIfIdentity can return changed=true together
			// with a post-commit housekeeping error; only changed=false proves
			// that the stop transition itself did not commit.
			return false, stateErr
		}
	}

	ownership, ok, ownershipErr := st.GetCgroupOwnership(c.ID)
	if ownershipErr != nil {
		ownershipErr = fmt.Errorf("read cgroup ownership for container %s: %w", c.ID, ownershipErr)
	} else if ok {
		if ownership.PID != c.PID || ownership.PIDStartTime != c.PIDStartTime {
			ownershipErr = fmt.Errorf(
				"cgroup ownership for container %s belongs to process %d/%d, not finalized generation %d/%d",
				c.ID,
				ownership.PID,
				ownership.PIDStartTime,
				c.PID,
				c.PIDStartTime,
			)
		} else if err := cleanupOwnedGenerationWith(st, c.ID, ownership, cleanup); err != nil {
			ownershipErr = fmt.Errorf("cleanup stopped process generation for container %s: %w", c.ID, err)
		}
	}

	return changed, errors.Join(stateErr, ownershipErr)
}
