package state

import (
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
)

func imageEnvironmentKey(selector string) string {
	sum := sha256.Sum256([]byte(selector))
	return ".image-environment-" + hex.EncodeToString(sum[:])
}

// SaveImageEnvironment persists OCI image environment independently from the
// image record so legacy callers that republish basic image metadata cannot
// accidentally erase runtime defaults.
func (s *Store) SaveImageEnvironment(selector string, env []string) error {
	if err := validateImageSelector(selector); err != nil {
		return err
	}
	data, err := json.Marshal(env)
	if err != nil {
		return fmt.Errorf("marshal image environment: %w", err)
	}

	s.mu.Lock()
	defer s.mu.Unlock()
	if err := lockStateFile(s.lockFile); err != nil {
		return err
	}
	defer func() { _ = unlockStateFile(s.lockFile) }()

	path := filepath.Join(s.imgDir, imageEnvironmentKey(selector))
	return atomicWriteFile(s.imgDir, path, append(data, '\n'))
}

// ImageEnvironment returns persisted image-level environment metadata. ok is
// false for images created before OCI runtime environment persistence existed.
func (s *Store) ImageEnvironment(selector string) (env []string, ok bool, err error) {
	if err := validateImageSelector(selector); err != nil {
		return nil, false, err
	}

	s.mu.Lock()
	defer s.mu.Unlock()
	if s.lockFile == nil {
		return nil, false, ErrStoreClosed
	}

	path := filepath.Join(s.imgDir, imageEnvironmentKey(selector))
	data, err := readRegularStateFile(path, "image environment")
	if err != nil {
		if os.IsNotExist(err) {
			return nil, false, nil
		}
		return nil, false, fmt.Errorf("read image environment: %w", err)
	}
	if err := json.Unmarshal(data, &env); err != nil {
		return nil, false, fmt.Errorf("decode image environment: %w", err)
	}
	return append([]string(nil), env...), true, nil
}
