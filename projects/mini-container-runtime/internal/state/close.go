package state

import (
	"errors"
	"fmt"
)

// Close releases the filesystem handles owned by the Store. Close serializes
// with state operations through the Store mutex, so an in-flight operation
// finishes before its pinned directories can be closed. It is safe to call
// Close more than once.
//
// A Store must not be used for state operations after Close returns.
func (s *Store) Close() error {
	if s == nil {
		return nil
	}

	s.mu.Lock()
	defer s.mu.Unlock()

	// Clearing each handle as it is consumed makes Close idempotent even when a
	// close reports an error. Retrying Close must never target a descriptor that
	// the OS may already have recycled for an unrelated resource.
	var errs []error
	if s.lockFile != nil {
		if err := s.lockFile.Close(); err != nil {
			errs = append(errs, fmt.Errorf("close state lock: %w", err))
		}
		s.lockFile = nil
	}
	for i := len(s.storagePins) - 1; i >= 0; i-- {
		if s.storagePins[i] == nil {
			continue
		}
		if err := s.storagePins[i].Close(); err != nil {
			errs = append(errs, fmt.Errorf("close pinned state directory: %w", err))
		}
		s.storagePins[i] = nil
	}
	s.storagePins = nil

	return errors.Join(errs...)
}
