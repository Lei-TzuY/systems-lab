package imagestore

import (
	"fmt"

	"minicontainer/internal/diff"
	"minicontainer/internal/state"
)

// DiffImages compares files between two image tags.
func DiffImages(st *state.Store, tag1, tag2 string) ([]diff.Change, error) {
	if st == nil {
		return nil, fmt.Errorf("state store is nil")
	}

	img1, err := st.GetImageUnlocked(tag1)
	if err != nil {
		return nil, fmt.Errorf("get image %s: %w", tag1, err)
	}
	_ = img1

	img2, err := st.GetImageUnlocked(tag2)
	if err != nil {
		return nil, fmt.Errorf("get image %s: %w", tag2, err)
	}

	return diff.DiffUpper(img2.RootFS)
}
