package events

import (
	"fmt"

	"minicontainer/internal/state"
)

// validatePersistedCreate makes durable container state the authority for a
// create event. A generated ID alone is not proof that creation succeeded: the
// initial state write may have failed, or the configured state-root pathname may
// have been replaced between the write and event publication.
func validatePersistedCreate(containerID string) error {
	if containerID == "" {
		return fmt.Errorf("create event requires a container ID")
	}

	st, err := state.Open(state.DefaultDir())
	if err != nil {
		return fmt.Errorf("open state store for create event: %w", err)
	}
	defer st.Close()

	rec, err := st.Get(containerID)
	if err != nil {
		return fmt.Errorf("verify durable create state for container %s: %w", containerID, err)
	}
	if rec.ID != containerID {
		return fmt.Errorf("durable create state identity mismatch: got %q, want %q", rec.ID, containerID)
	}
	return nil
}
