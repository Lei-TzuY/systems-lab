package imagestore

import (
	"fmt"
	"time"

	"minicontainer/internal/state"
)

// CreateTagAlias creates a new tag alias for an existing image.
func CreateTagAlias(st *state.Store, srcTag, targetTag string) (*state.Image, error) {
	if st == nil {
		return nil, fmt.Errorf("state store is nil")
	}

	src, err := st.GetImageUnlocked(srcTag)
	if err != nil {
		return nil, fmt.Errorf("get src image %s: %w", srcTag, err)
	}

	newImg := &state.Image{
		ID:           src.ID,
		Name:         targetTag,
		Tag:          targetTag,
		RootFS:       src.RootFS,
		Size:         src.Size,
		LoadedAt:     time.Now(),
		WorkDir:      src.WorkDir,
		Env:          src.Env,
		Cmd:          src.Cmd,
		ExposedPorts: src.ExposedPorts,
	}

	if err := st.PublishImageIfSourceMatch(srcTag, src, newImg); err != nil {
		return nil, fmt.Errorf("save tagged image: %w", err)
	}

	return newImg, nil
}
