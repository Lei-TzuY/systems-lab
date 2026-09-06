package state

import (
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
)

func imageWorkingDirKey(selector string) string {
	sum := sha256.Sum256([]byte(selector))
	return ".image-working-dir-" + hex.EncodeToString(sum[:])
}

// SaveImageWorkingDir persists OCI image WorkingDir independently from the
// image record so legacy callers that republish basic image metadata cannot
// accidentally erase the runtime default.
func (s *Store) SaveImageWorkingDir(selector, workDir string) error {
	if err := validateImageSelector(selector); err != nil {
		return err
	}
	data, err := json.Marshal(workDir)
	if err != nil {
		return fmt.Errorf("marshal image WorkingDir: %w", err)
	}

	s.mu.Lock()
	defer s.mu.Unlock()
	if err := lockStateFile(s.lockFile); err != nil {
		return err
	}
	defer func() { _ = unlockStateFile(s.lockFile) }()

	path := filepath.Join(s.imgDir, imageWorkingDirKey(selector))
	return atomicWriteFile(s.imgDir, path, append(data, '\n'))
}

// ImageWorkingDir returns persisted image-level WorkingDir metadata. ok is
// false for images created before OCI WorkingDir persistence existed.
func (s *Store) ImageWorkingDir(selector string) (workDir string, ok bool, err error) {
	if err := validateImageSelector(selector); err != nil {
		return "", false, err
	}

	s.mu.Lock()
	defer s.mu.Unlock()
	if s.lockFile == nil {
		return "", false, ErrStoreClosed
	}

	path := filepath.Join(s.imgDir, imageWorkingDirKey(selector))
	data, err := readRegularStateFile(path, "image WorkingDir")
	if err != nil {
		if os.IsNotExist(err) {
			return "", false, nil
		}
		return "", false, fmt.Errorf("read image WorkingDir: %w", err)
	}
	if err := json.Unmarshal(data, &workDir); err != nil {
		return "", false, fmt.Errorf("decode image WorkingDir: %w", err)
	}
	return workDir, true, nil
}
