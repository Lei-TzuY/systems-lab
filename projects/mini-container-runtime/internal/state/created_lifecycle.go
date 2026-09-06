package state

import (
	"fmt"
	"time"
)

// MarkStoppedIfCreated atomically records a startup failure only while the
// container is still in its pre-process created state. It deliberately refuses
// to overwrite a running/stopped lifecycle or any created record that already
// carries process, cgroup, network, or exited-generation evidence.
//
// Returning changed=false means another lifecycle actor already moved the
// record away from created; callers must reload and trust that newer state.
func (s *Store) MarkStoppedIfCreated(id string, exitCode int, finishedAt time.Time) (changed bool, err error) {
	if s == nil {
		return false, fmt.Errorf("state store is nil")
	}
	if err := validateID(id); err != nil {
		return false, err
	}

	s.mu.Lock()
	defer s.mu.Unlock()
	if err := lockStateFile(s.lockFile); err != nil {
		return false, err
	}
	defer func() { _ = unlockStateFile(s.lockFile) }()

	c, err := s.getUnlocked(id)
	if err != nil {
		return false, err
	}
	if c.Status != StatusCreated {
		return false, nil
	}
	if c.PID != 0 || c.PIDStartTime != 0 {
		return false, fmt.Errorf(
			"created container %s unexpectedly carries process identity %d/%d",
			id,
			c.PID,
			c.PIDStartTime,
		)
	}
	ownership, ok, err := s.readCgroupOwnershipUnlocked(id)
	if err != nil {
		return false, fmt.Errorf("read cgroup ownership before recording startup failure for container %s: %w", id, err)
	}
	if ok {
		return false, fmt.Errorf(
			"created container %s has cgroup ownership for %s (%d/%d); refusing synthetic stop",
			id,
			ownership.Name,
			ownership.PID,
			ownership.PIDStartTime,
		)
	}
	networkOwnership, ok, err := s.readNetworkOwnershipUnlocked(id)
	if err != nil {
		return false, fmt.Errorf("read network ownership before recording startup failure for container %s: %w", id, err)
	}
	if ok {
		return false, fmt.Errorf(
			"created container %s has network ownership for %s (%d/%d); refusing synthetic stop",
			id,
			networkOwnership.Owner,
			networkOwnership.PID,
			networkOwnership.PIDStartTime,
		)
	}
	exited, ok, err := s.readExitedIdentityUnlocked(id)
	if err != nil {
		return false, fmt.Errorf("read exited identity before recording startup failure for container %s: %w", id, err)
	}
	if ok {
		return false, fmt.Errorf(
			"created container %s has exited-generation identity %d/%d; refusing synthetic stop",
			id,
			exited.PID,
			exited.PIDStartTime,
		)
	}

	c.Status = StatusStopped
	c.FinishedAt = &finishedAt
	c.ExitCode = exitCode
	if err := s.writeContainerNextRevisionUnlocked(c); err != nil {
		return false, err
	}
	return true, nil
}
