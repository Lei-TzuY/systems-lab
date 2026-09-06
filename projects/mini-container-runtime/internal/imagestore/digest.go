package imagestore

import (
	"fmt"
	"strings"

	"minicontainer/internal/state"
)

// SearchImageByDigest searches imagestore for images matching a specific layer SHA-256 digest.
func SearchImageByDigest(st *state.Store, digestHash string) ([]*state.Image, error) {
	if st == nil {
		return nil, fmt.Errorf("state store is nil")
	}

	images, err := st.ListImages()
	if err != nil {
		return nil, err
	}

	var matches []*state.Image
	for _, img := range images {
		if strings.Contains(img.ID, digestHash) || strings.Contains(img.Tag, digestHash) {
			matches = append(matches, img)
		}
	}

	return matches, nil
}
