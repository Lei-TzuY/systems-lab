package state

import (
	"errors"
	"fmt"
	"os"
	"path/filepath"
)

// ClearImageCleanupIfReferenced atomically decides whether recovery must stop
// before payload deletion. It returns true in either safe-stop case:
//   - durable metadata still references the owned rootfs, in which case this
//     method clears the stale sidecar while holding the state lock; or
//   - the exact sidecar no longer exists, meaning another actor already retired
//     the cleanup authority and this caller must not continue destructively.
//
// False means the exact sidecar still exists and no metadata currently
// references its rootfs, so the caller retains durable authority to finish
// payload cleanup. The reference check and sidecar state share one process/file
// lock and therefore cannot be separated by a metadata mutation.
func (s *Store) ClearImageCleanupIfReferenced(cleanup ImageCleanup) (bool, error) {
	if s == nil {
		return false, fmt.Errorf("state store is nil")
	}
	cleanup, err := normalizeImageCleanup(cleanup)
	if err != nil {
		return false, err
	}

	s.mu.Lock()
	defer s.mu.Unlock()
	if err := lockStateFile(s.lockFile); err != nil {
		return false, err
	}
	defer func() { _ = unlockStateFile(s.lockFile) }()

	path := imageCleanupPath(s.imgDir, cleanup)
	persisted, err := readImageCleanup(path)
	if err != nil {
		if errors.Is(err, os.ErrNotExist) {
			// No durable cleanup proof means no destructive authority. Another
			// recovery actor may already have retired this exact sidecar.
			return true, nil
		}
		return false, fmt.Errorf("read pending image cleanup before reference proof: %w", err)
	}
	if persisted != cleanup {
		return false, fmt.Errorf("pending image cleanup changed before reference proof")
	}

	images, err := s.listImagesUnlocked()
	if err != nil {
		return false, err
	}
	for _, img := range images {
		if img == nil {
			continue
		}
		rootFS := filepath.Clean(img.RootFS)
		if img.ID == cleanup.ID && rootFS != cleanup.RootFS {
			return false, fmt.Errorf("image ID %s points at %q while pending cleanup owns %q", cleanup.ID, rootFS, cleanup.RootFS)
		}
		if rootFS == cleanup.RootFS {
			cleared, err := s.clearImageCleanupUnlocked(cleanup)
			if err != nil {
				return false, err
			}
			if !cleared {
				// Another actor removed the token while we held no in-process lock
				// before this call. Either way, this caller must not delete payload.
				return true, nil
			}
			return true, nil
		}
	}
	return false, nil
}
