package state

import (
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
)

// ImageCommand captures OCI image process defaults independently from the
// general image record so later basic metadata republishes cannot erase them.
type ImageCommand struct {
	Entrypoint []string `json:"entrypoint,omitempty"`
	Cmd        []string `json:"cmd,omitempty"`
}

func imageCommandKey(selector string) string {
	sum := sha256.Sum256([]byte(selector))
	return ".image-command-" + hex.EncodeToString(sum[:])
}

// SaveImageCommand persists OCI Entrypoint/Cmd as durable image runtime metadata.
func (s *Store) SaveImageCommand(selector string, command ImageCommand) error {
	if err := validateImageSelector(selector); err != nil {
		return err
	}
	data, err := json.Marshal(ImageCommand{
		Entrypoint: append([]string(nil), command.Entrypoint...),
		Cmd:        append([]string(nil), command.Cmd...),
	})
	if err != nil {
		return fmt.Errorf("marshal image command: %w", err)
	}

	s.mu.Lock()
	defer s.mu.Unlock()
	if err := lockStateFile(s.lockFile); err != nil {
		return err
	}
	defer func() { _ = unlockStateFile(s.lockFile) }()

	path := filepath.Join(s.imgDir, imageCommandKey(selector))
	return atomicWriteFile(s.imgDir, path, append(data, '\n'))
}

// ImageCommandConfig returns persisted OCI image process defaults. ok is false
// for images created before command metadata persistence existed.
func (s *Store) ImageCommandConfig(selector string) (command ImageCommand, ok bool, err error) {
	if err := validateImageSelector(selector); err != nil {
		return ImageCommand{}, false, err
	}

	s.mu.Lock()
	defer s.mu.Unlock()
	if s.lockFile == nil {
		return ImageCommand{}, false, ErrStoreClosed
	}

	path := filepath.Join(s.imgDir, imageCommandKey(selector))
	data, err := readRegularStateFile(path, "image command")
	if err != nil {
		if os.IsNotExist(err) {
			return ImageCommand{}, false, nil
		}
		return ImageCommand{}, false, fmt.Errorf("read image command: %w", err)
	}
	if err := json.Unmarshal(data, &command); err != nil {
		return ImageCommand{}, false, fmt.Errorf("decode image command: %w", err)
	}
	command.Entrypoint = append([]string(nil), command.Entrypoint...)
	command.Cmd = append([]string(nil), command.Cmd...)
	return command, true, nil
}
