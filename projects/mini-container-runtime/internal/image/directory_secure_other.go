//go:build !linux

package image

import (
	"fmt"
	"os"
)

func createDirectorySecure(target, destDir string, mode os.FileMode) error {
	if fi, err := os.Lstat(target); err == nil && fi.Mode()&os.ModeSymlink != 0 {
		if err := os.Remove(target); err != nil && !os.IsNotExist(err) {
			return fmt.Errorf("remove existing symlink before mkdir %s: %w", target, err)
		}
	}
	return os.MkdirAll(target, mode)
}
