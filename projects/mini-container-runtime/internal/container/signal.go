package container

import (
	"fmt"

	"minicontainer/internal/state"
)

// SendSignal sends a custom signal to the exact process identity persisted for
// a running container. It never falls back to signaling a raw numeric PID.
func SendSignal(st *state.Store, containerID, sigName string) error {
	_, err := SendSignalResolved(st, containerID, sigName)
	return err
}

// SendSignalResolved sends a custom signal to the exact persisted process
// generation and returns the canonical state snapshot used for that pidfd
// operation. Callers that need audit metadata should use this snapshot instead
// of resolving state again after the signal, because the state record may be
// concurrently reconciled or deleted after the signal has already succeeded.
func SendSignalResolved(st *state.Store, containerID, sigName string) (*state.Container, error) {
	c, handle, err := openRunningProcess(st, containerID)
	if err != nil {
		return nil, err
	}
	defer handle.Close()

	sig, err := ParseSignal(sigName)
	if err != nil {
		return nil, err
	}
	if err := handle.Signal(sig); err != nil {
		shortID := c.ID
		if len(shortID) > 8 {
			shortID = shortID[:8]
		}
		return nil, fmt.Errorf("signal container %s: %w", shortID, err)
	}
	return c, nil
}
