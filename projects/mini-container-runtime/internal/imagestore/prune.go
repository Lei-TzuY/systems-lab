package imagestore

import (
	"errors"
	"fmt"
	"os"

	"minicontainer/internal/state"
)

// PruneOrphanLayers scans the imagestore and removes dangling image metadata
// together with payloads that are no longer referenced by another image alias.
func PruneOrphanLayers(st *state.Store) (int, int64, error) {
	if st == nil {
		return 0, 0, fmt.Errorf("state store is nil")
	}
	if err := recoverPendingManagedImageCleanups(st); err != nil {
		return 0, 0, fmt.Errorf("recover pending image cleanup before prune: %w", err)
	}

	images, err := st.ListImages()
	if err != nil {
		return 0, 0, err
	}

	var count int
	var reclaimedBytes int64

	for _, img := range images {
		if img == nil || (img.Tag != "" && img.Tag != "<none>") {
			continue
		}

		targetKey := img.Name
		if targetKey == "" {
			targetKey = img.ID
		}

		// Track whether a real payload existed before removal. This prevents an
		// already-missing rootfs from being reported as newly reclaimed space.
		hadPayload := false
		if img.RootFS != "" {
			info, statErr := os.Lstat(img.RootFS)
			switch {
			case statErr == nil:
				hadPayload = info.IsDir()
			case errors.Is(statErr, os.ErrNotExist):
				// Missing payloads still have dangling metadata to prune, but no
				// bytes were reclaimed by this operation.
			default:
				return count, reclaimedBytes, fmt.Errorf("inspect dangling image %q rootfs before prune: %w", targetKey, statErr)
			}
		}

		removed, err := RemoveImage(st, targetKey, true)
		if err != nil {
			return count, reclaimedBytes, fmt.Errorf("prune dangling image %q: %w", targetKey, err)
		}
		count++

		if !hadPayload || removed == nil || removed.RootFS == "" {
			continue
		}
		if _, statErr := os.Lstat(removed.RootFS); statErr == nil {
			// Another alias still references the payload (or a concurrent owner
			// recreated it), so pruning this metadata did not reclaim its bytes.
			continue
		} else if errors.Is(statErr, os.ErrNotExist) {
			reclaimedBytes += removed.Size
		} else {
			return count, reclaimedBytes, fmt.Errorf("verify dangling image %q rootfs after prune: %w", targetKey, statErr)
		}
	}

	return count, reclaimedBytes, nil
}
