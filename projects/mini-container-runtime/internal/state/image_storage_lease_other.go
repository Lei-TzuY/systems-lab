//go:build !linux

package state

import (
	"fmt"
	"os"
)

func acquireImageStorageLeaseLocked(s *Store) (*ImageStorageLease, error) {
	rootFile, err := os.Open(s.dir)
	if err != nil {
		return nil, fmt.Errorf("open state root lease: %w", err)
	}
	imageFile, err := os.Open(s.imgDir)
	if err != nil {
		_ = rootFile.Close()
		return nil, fmt.Errorf("open image directory lease: %w", err)
	}
	return newImageStorageLease(rootFile, imageFile, s.imgDir, s.dir), nil
}
