package state

import (
	"fmt"
	"time"
)

// MarkRunning atomically transitions an existing container record to running
// and binds it to a specific host process identity (PID + Linux starttime).
// A stopped container cannot start while any durable cleanup ownership from its
// previous generation remains pending.
func (s *Store) MarkRunning(id string, pid int, pidStartTime uint64, startedAt time.Time) error {
	if s == nil {
		return fmt.Errorf("state store is nil")
	}
	if err := validateID(id); err != nil {
		return err
	}
	if pid <= 0 {
		return fmt.Errorf("invalid container PID %d", pid)
	}
	if pidStartTime == 0 {
		return fmt.Errorf("process start time must be non-zero")
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
	if c.Status == StatusRunning && (c.PID != pid || c.PIDStartTime != pidStartTime) {
		return fmt.Errorf("container %s is already bound to running process %d/%d", id, c.PID, c.PIDStartTime)
	}
	if c.Status != StatusRunning {
		ownership, ok, err := s.readCgroupOwnershipUnlocked(id)
		if err != nil {
			return fmt.Errorf("read pending cgroup ownership before starting container %s: %w", id, err)
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
			return fmt.Errorf("read pending network ownership before starting container %s: %w", id, err)
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
	}

	c.PID = pid
	c.PIDStartTime = pidStartTime
	c.Status = StatusRunning
	c.StartedAt = &startedAt
	c.FinishedAt = nil
	c.ExitCode = 0

	if err := s.writeContainerNextRevisionUnlocked(c); err != nil {
		return err
	}
	// Modern running JSON no longer contains stopped-generation authority. Clear
	// any upgrade-era sidecar only after the new generation is durable.
	_ = s.clearExitedIdentityUnlocked(id)
	return nil
}

// MarkStoppedIfIdentity atomically marks a running container stopped only when
// the persisted process identity still matches the caller's observation.
//
// Every successful modern stop commits stopped status, revision, capability,
// and exact PID/start-time teardown key in one atomic container JSON replace.
// If an observer knows only that the process exited, it may persist exitCode -1
// and the process-owning reaper can later upgrade that code only for the same
// PID/start-time lifecycle.
//
// Returning changed=false is intentional: another lifecycle actor may already
// have stopped/restarted the container, and stale observations must not win.
func (s *Store) MarkStoppedIfIdentity(id string, pid int, pidStartTime uint64, exitCode int, finishedAt time.Time) (changed bool, err error) {
	if s == nil {
		return false, fmt.Errorf("state store is nil")
	}
	if err := validateID(id); err != nil {
		return false, err
	}
	if pid <= 0 || pidStartTime == 0 {
		return false, fmt.Errorf("invalid process identity %d/%d", pid, pidStartTime)
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

	// A non-owning observer may have won the stopped-state race with an unknown
	// exit code. Permit one later authoritative upgrade only when the durable
	// exited identity proves it refers to the same process generation.
	if c.Status == StatusStopped {
		if c.ExitCode != -1 || exitCode == -1 {
			return false, nil
		}
		exited, ok, err := s.readCurrentExitedIdentityUnlocked(id)
		if err != nil {
			return false, fmt.Errorf("read exited identity for exit-code reconciliation: %w", err)
		}
		if !ok || exited.PID != pid || exited.PIDStartTime != pidStartTime {
			return false, nil
		}

		c.FinishedAt = &finishedAt
		c.ExitCode = exitCode
		if err := s.writeContainerNextRevisionUnlocked(c); err != nil {
			return false, err
		}
		return true, nil
	}

	if c.Status != StatusRunning || c.PID != pid || c.PIDStartTime != pidStartTime {
		return false, nil
	}

	c.Status = StatusStopped
	c.PID = 0
	c.PIDStartTime = 0
	c.FinishedAt = &finishedAt
	c.ExitCode = exitCode

	if err := s.writeStoppedContainerNextRevisionUnlocked(c, pid, pidStartTime); err != nil {
		return false, err
	}
	// An upgrade-era sidecar, if one somehow survived a prior running transition,
	// is no longer authoritative once the embedded identity is durable.
	_ = s.clearExitedIdentityUnlocked(id)
	return true, nil
}
