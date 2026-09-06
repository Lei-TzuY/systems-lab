package state

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"
)

const defaultContainerStopSignal = "SIGTERM"

// SaveContainerStopSignal persists the graceful stop signal selected for a
// container. The value is stored beside the container record so lifecycle
// control can honor it independently of the process that originally admitted
// the run.
func (s *Store) SaveContainerStopSignal(id, signal string) error {
	if err := validateID(id); err != nil {
		return err
	}
	signal = strings.TrimSpace(signal)
	if signal == "" {
		return fmt.Errorf("container stop signal cannot be empty")
	}

	s.mu.Lock()
	defer s.mu.Unlock()
	if err := lockStateFile(s.lockFile); err != nil {
		return err
	}
	defer func() { _ = unlockStateFile(s.lockFile) }()

	path := filepath.Join(s.ctrDir, id+".stop-signal")
	return atomicWriteFile(s.ctrDir, path, []byte(signal+"\n"))
}

// ContainerStopSignal returns the persisted graceful stop signal. Containers
// created before stop-signal persistence retain the historical SIGTERM default.
func (s *Store) ContainerStopSignal(id string) (string, error) {
	if err := validateID(id); err != nil {
		return "", err
	}

	s.mu.Lock()
	defer s.mu.Unlock()
	if s.lockFile == nil {
		return "", ErrStoreClosed
	}

	path := filepath.Join(s.ctrDir, id+".stop-signal")
	data, err := readRegularStateFile(path, "container stop signal")
	if err != nil {
		if os.IsNotExist(err) {
			return defaultContainerStopSignal, nil
		}
		return "", fmt.Errorf("read container stop signal: %w", err)
	}
	signal := strings.TrimSpace(string(data))
	if signal == "" {
		return "", fmt.Errorf("container %s has empty persisted stop signal", id)
	}
	return signal, nil
}
