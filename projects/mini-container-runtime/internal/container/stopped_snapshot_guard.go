package container

import (
	"fmt"

	"minicontainer/internal/state"
)

// stoppedSnapshotStillCurrent prevents a delayed cleanup actor from consuming
// durable resource ownership created by a later lifecycle generation. Status
// alone is insufficient: an old stopped snapshot can outlive a restart and run
// again after the newer generation has also stopped. Revision is the durable
// lifecycle CAS token that distinguishes those two stopped states.
func stoppedSnapshotStillCurrent(st *state.Store, snapshot *state.Container) (bool, error) {
	if st == nil {
		return false, fmt.Errorf("state store is nil")
	}
	if snapshot == nil {
		return false, fmt.Errorf("container snapshot is nil")
	}
	if snapshot.ID == "" {
		return false, fmt.Errorf("container ID is empty")
	}
	current, err := st.Get(snapshot.ID)
	if err != nil {
		return false, fmt.Errorf("reload stopped lifecycle state for container %s: %w", snapshot.ID, err)
	}
	if current.Status != state.StatusStopped {
		return false, nil
	}
	return current.Revision == snapshot.Revision, nil
}
