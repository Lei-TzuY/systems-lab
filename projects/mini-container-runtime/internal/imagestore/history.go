package imagestore

import (
	"fmt"
	"time"

	"minicontainer/internal/state"
)

type ImageHistoryLayer struct {
	ID        string    `json:"id"`
	CreatedBy string    `json:"created_by"`
	Size      int64     `json:"size"`
	CreatedAt time.Time `json:"created_at"`
}

// GetImageHistory audits layer build history and metadata for an image tag.
func GetImageHistory(st *state.Store, tag string) ([]ImageHistoryLayer, error) {
	if st == nil {
		return nil, fmt.Errorf("state store is nil")
	}

	img, err := st.GetImageUnlocked(tag)
	if err != nil {
		return nil, fmt.Errorf("get image %s: %w", tag, err)
	}

	layers := []ImageHistoryLayer{
		{
			ID:        img.ID[:min(12, len(img.ID))],
			CreatedBy: fmt.Sprintf("CMD %v", img.Cmd),
			Size:      img.Size,
			CreatedAt: img.LoadedAt,
		},
	}

	return layers, nil
}

func min(a, b int) int {
	if a < b {
		return a
	}
	return b
}
