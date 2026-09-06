package container

import (
	"fmt"

	"minicontainer/internal/state"
)

// RenameContainer updates the persisted hostname only when no live process
// generation is using the old UTS hostname. Running containers must be stopped
// first because changing metadata alone cannot rename their active namespace.
func RenameContainer(st *state.Store, containerID, newName string) error {
	if st == nil {
		return fmt.Errorf("state store is nil")
	}
	if newName == "" {
		return fmt.Errorf("new container name cannot be empty")
	}

	c, err := st.Resolve(containerID)
	if err != nil {
		return fmt.Errorf("resolve container: %w", err)
	}
	if err := st.SetHostnameIfNotRunning(c.ID, newName); err != nil {
		return fmt.Errorf("rename container %s: %w", c.ID, err)
	}
	return nil
}
