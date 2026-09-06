package state

import (
	"errors"
	"fmt"
)

// WithRunningGenerationLocked serializes an operation against all durable state
// mutations in this store while proving that the current on-disk record still
// refers to the expected running process generation. The callback must not call
// Store methods on s, because the store mutex and cross-process state lock are
// held for its duration.
func (s *Store) WithRunningGenerationLocked(
	id string,
	pid int,
	pidStartTime uint64,
	fn func() error,
) (resultErr error) {
	if s == nil {
		return fmt.Errorf("state store is nil")
	}
	if err := validateID(id); err != nil {
		return err
	}
	if pid <= 0 || pidStartTime == 0 {
		return fmt.Errorf("container %s has invalid process generation %d/%d", id, pid, pidStartTime)
	}
	if fn == nil {
		return fmt.Errorf("running generation callback is nil")
	}

	s.mu.Lock()
	defer s.mu.Unlock()
	if s.lockFile == nil {
		return ErrStoreClosed
	}
	if err := lockStateFile(s.lockFile); err != nil {
		return err
	}
	defer func() {
		if err := unlockStateFile(s.lockFile); err != nil {
			resultErr = errors.Join(resultErr, err)
		}
	}()

	current, err := s.getUnlocked(id)
	if err != nil {
		return fmt.Errorf("load current container %s under generation lock: %w", id, err)
	}
	if current.Status != StatusRunning || current.PID != pid || current.PIDStartTime != pidStartTime {
		return fmt.Errorf(
			"container %s running generation changed: expected %d/%d, found status=%s generation=%d/%d",
			id,
			pid,
			pidStartTime,
			current.Status,
			current.PID,
			current.PIDStartTime,
		)
	}
	return fn()
}
