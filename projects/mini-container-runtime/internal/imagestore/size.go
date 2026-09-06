package imagestore

import (
	"fmt"
	"io/fs"
	"path/filepath"

	"minicontainer/internal/state"
)

// CalculateImageSize recursively measures rootfs disk usage in bytes.
func CalculateImageSize(st *state.Store, tag string) (int64, error) {
	if st == nil {
		return 0, fmt.Errorf("state store is nil")
	}

	img, err := st.GetImageUnlocked(tag)
	if err != nil {
		return 0, fmt.Errorf("get image %s: %w", tag, err)
	}

	var totalSize int64
	err = filepath.WalkDir(img.RootFS, func(path string, d fs.DirEntry, err error) error {
		if err != nil {
			return nil
		}
		if info, err := d.Info(); err == nil && !info.IsDir() {
			totalSize += info.Size()
		}
		return nil
	})

	return totalSize, err
}
