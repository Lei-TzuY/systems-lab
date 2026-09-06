//go:build !linux

package image

import (
	"archive/tar"
	"fmt"
	"os"
)

func createSymlinkSecure(target, destDir, linkname string) error {
	_ = destDir
	if err := os.RemoveAll(target); err != nil && !os.IsNotExist(err) {
		return fmt.Errorf("remove existing node before symlink %s: %w", target, err)
	}
	if err := os.Symlink(linkname, target); err != nil && !os.IsExist(err) {
		return fmt.Errorf("symlink %s → %s: %w", target, linkname, err)
	}
	return nil
}

func createTarSymlinkSecure(target, destDir string, hdr *tar.Header) error {
	if hdr == nil {
		return fmt.Errorf("symlink tar header is nil")
	}
	return createSymlinkSecure(target, destDir, hdr.Linkname)
}
