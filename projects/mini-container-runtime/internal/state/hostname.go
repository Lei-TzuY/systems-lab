package state

import "fmt"

// SetHostnameIfNotRunning updates only the persisted hostname while holding the
// state mutation lock. A running container's UTS hostname is already fixed in
// its active namespace, so changing only metadata would make inspect/state
// disagree with the live container. Callers must stop the container first.
func (s *Store) SetHostnameIfNotRunning(id, hostname string) error {
	if s == nil {
		return fmt.Errorf("state store is nil")
	}
	if err := validateID(id); err != nil {
		return err
	}
	if hostname == "" {
		return fmt.Errorf("hostname cannot be empty")
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
	switch c.Status {
	case StatusRunning:
		return fmt.Errorf("container %s is running; stop it before renaming", id)
	case StatusCreated, StatusStopped:
		// Safe metadata-only states.
	default:
		return fmt.Errorf("container %s has unknown lifecycle status %q; refusing rename", id, c.Status)
	}
	if c.Hostname == hostname {
		return nil
	}

	c.Hostname = hostname
	if err := s.writeContainerNextRevisionUnlocked(c); err != nil {
		return fmt.Errorf("persist hostname for container %s: %w", id, err)
	}
	return nil
}
