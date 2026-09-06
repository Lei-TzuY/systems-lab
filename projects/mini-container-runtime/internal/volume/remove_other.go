//go:build !linux

package volume

import (
	"os"
	"path/filepath"
)

func removeVolumeDir(root, name string) error {
	return os.RemoveAll(filepath.Join(root, name))
}
