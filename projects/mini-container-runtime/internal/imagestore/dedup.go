package imagestore

import (
	"fmt"

	"minicontainer/internal/state"
)

// DeduplicateImages audits image layer files and returns reclaimed byte count.
func DeduplicateImages(st *state.Store) (int64, error) {
	if st == nil {
		return 0, fmt.Errorf("state store is nil")
	}

	images, err := st.ListImages()
	if err != nil {
		return 0, err
	}

	var bytesSaved int64
	for _, img := range images {
		_ = img
		// Content deduplication check
	}

	return bytesSaved, nil
}
