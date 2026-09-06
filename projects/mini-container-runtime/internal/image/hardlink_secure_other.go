//go:build !linux

package image

import (
	"archive/tar"
	"fmt"
	"os"
)

func createHardlinkSecure(target, destDir, linkTarget string) error {
	_ = destDir
	if err := os.RemoveAll(target); err != nil && !os.IsNotExist(err) {
		return fmt.Errorf("remove existing node before hardlink %s: %w", target, err)
	}
	if err := os.Link(linkTarget, target); err != nil {
		return fmt.Errorf("hardlink %s → %s: %w", target, linkTarget, err)
	}
	return nil
}

func createTarHardlinkSecure(target, destDir, linkTarget string, hdr *tar.Header) error {
	_ = hdr
	return createHardlinkSecure(target, destDir, linkTarget)
}
