package state

import (
	"crypto/sha256"
	"encoding/hex"
	"fmt"
	"os"
	"path/filepath"
	"strings"
)

func imageStopSignalKey(selector string) string {
	sum := sha256.Sum256([]byte(selector))
	return ".image-stop-signal-" + hex.EncodeToString(sum[:])
}

// SaveImageStopSignal persists the canonical graceful stop signal associated
// with one registered image selector without changing the image metadata schema.
func (s *Store) SaveImageStopSignal(selector, signal string) error {
	if err := validateImageSelector(selector); err != nil {
		return err
	}
	signal = strings.TrimSpace(signal)
	if signal == "" {
		return fmt.Errorf("image stop signal cannot be empty")
	}

	s.mu.Lock()
	defer s.mu.Unlock()
	if err := lockStateFile(s.lockFile); err != nil {
		return err
	}
	defer func() { _ = unlockStateFile(s.lockFile) }()

	path := filepath.Join(s.imgDir, imageStopSignalKey(selector))
	return atomicWriteFile(s.imgDir, path, []byte(signal+"\n"))
}

// ImageStopSignal returns a persisted image-level stop signal. ok is false for
// legacy images that do not yet have OCI StopSignal metadata.
func (s *Store) ImageStopSignal(selector string) (signal string, ok bool, err error) {
	if err := validateImageSelector(selector); err != nil {
		return "", false, err
	}

	s.mu.Lock()
	defer s.mu.Unlock()
	if s.lockFile == nil {
		return "", false, ErrStoreClosed
	}

	path := filepath.Join(s.imgDir, imageStopSignalKey(selector))
	data, err := readRegularStateFile(path, "image stop signal")
	if err != nil {
		if os.IsNotExist(err) {
			return "", false, nil
		}
		return "", false, fmt.Errorf("read image stop signal: %w", err)
	}
	signal = strings.TrimSpace(string(data))
	if signal == "" {
		return "", false, fmt.Errorf("image %q has empty persisted stop signal", selector)
	}
	return signal, true, nil
}
