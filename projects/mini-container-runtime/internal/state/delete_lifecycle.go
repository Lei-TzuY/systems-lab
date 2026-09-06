package state

import (
	"errors"
	"fmt"
	"path/filepath"
)

// DeleteIfNotRunning atomically verifies the current on-disk lifecycle state
// and removes the container record only when it is not running. The status
// check, pending durable runtime-ownership checks, sidecar cleanup, and file
// deletion are serialized under the same process and cross-process state locks,
// closing the reconcile/delete race where another actor could restart a
// container after a caller observed it stopped but before state deletion.
func (s *Store) DeleteIfNotRunning(id string) error {
	if s == nil {
		return fmt.Errorf("state store is nil")
	}
	if err := validateID(id); err != nil {
		return err
	}

	s.mu.Lock()
	defer s.mu.Unlock()
	if err := lockStateFile(s.lockFile); err != nil {
		return err
	}
	defer func() { _ = unlockStateFile(s.lockFile) }()

	c, err := s.getUnlocked(id)
	if err != nil {
		return err
	}
	if c.Status == StatusRunning {
		return fmt.Errorf(
			"%w: container %s has process %d/%d; refusing deletion",
			ErrContainerRunning,
			id,
			c.PID,
			c.PIDStartTime,
		)
	}

	ownership, ok, err := s.readCgroupOwnershipUnlocked(id)
	if err != nil {
		return fmt.Errorf("read pending cgroup ownership before deleting container %s: %w", id, err)
	}
	if ok {
		return fmt.Errorf(
			"container %s has pending cgroup cleanup for %s (%d/%d)",
			id,
			ownership.Name,
			ownership.PID,
			ownership.PIDStartTime,
		)
	}

	networkOwnership, ok, err := s.readNetworkOwnershipUnlocked(id)
	if err != nil {
		return fmt.Errorf("read pending network ownership before deleting container %s: %w", id, err)
	}
	if ok {
		return fmt.Errorf(
			"container %s has pending network cleanup for %s (%d/%d)",
			id,
			networkOwnership.Owner,
			networkOwnership.PID,
			networkOwnership.PIDStartTime,
		)
	}

	// Unknown-exit reconciliation leaves a private identity tombstone so the
	// process-owning parent can later upgrade the exit code. Once the container
	// itself is being deleted that proof must not become an orphan. Clear it
	// before removing JSON, but retain enough data to restore it if the JSON
	// deletion fails so a failed delete cannot silently lose reconciliation state.
	exited, hasExited, err := s.readExitedIdentityUnlocked(id)
	if err != nil {
		return fmt.Errorf("read exited identity before deleting container %s: %w", id, err)
	}
	if hasExited {
		if err := s.clearExitedIdentityUnlocked(id); err != nil {
			return fmt.Errorf("clear exited identity before deleting container %s: %w", id, err)
		}
	}

	file := filepath.Join(s.ctrDir, id+".json")
	if err := removeStateFileDurable(s.ctrDir, file, "container state"); err != nil {
		if hasExited {
			restoreErr := s.writeExitedIdentityUnlocked(id, exited.PID, exited.PIDStartTime)
			if restoreErr != nil {
				return errors.Join(err, fmt.Errorf("restore exited identity after failed container deletion: %w", restoreErr))
			}
		}
		return err
	}
	return nil
}
