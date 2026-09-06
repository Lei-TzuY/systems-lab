//go:build linux

package state

import (
	"fmt"
	"os"
)

func acquireImageStorageLeaseLocked(s *Store) (*ImageStorageLease, error) {
	if len(s.storagePins) < 3 || s.storagePins[0] == nil || s.storagePins[2] == nil {
		return nil, fmt.Errorf("pinned state storage is unavailable")
	}

	rootFile, err := os.Open(procFDPath(s.storagePins[0]))
	if err != nil {
		return nil, fmt.Errorf("duplicate pinned state root: %w", err)
	}
	imageFile, err := os.Open(procFDPath(s.storagePins[2]))
	if err != nil {
		_ = rootFile.Close()
		return nil, fmt.Errorf("duplicate pinned image directory: %w", err)
	}
	return newImageStorageLease(rootFile, imageFile, procFDPath(imageFile), s.dir), nil
}
