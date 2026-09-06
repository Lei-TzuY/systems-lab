//go:build !linux

package image

import (
	"os"
	"path/filepath"
)

func removeWhiteoutSecure(target, destDir string) error {
	if err := ensureSafeParentDirs(target, destDir); err != nil {
		return err
	}
	return os.RemoveAll(target)
}

func clearOpaqueWhiteoutSecure(targetDir, destDir string) error {
	if err := ensureSafeParentDirs(targetDir, destDir); err != nil {
		return err
	}
	entries, err := os.ReadDir(targetDir)
	if os.IsNotExist(err) {
		return nil
	}
	if err != nil {
		return nil
	}
	for _, entry := range entries {
		if err := os.RemoveAll(filepath.Join(targetDir, entry.Name())); err != nil {
			return err
		}
	}
	return nil
}
