package registry

import (
	"errors"
	"fmt"
	"os"
	"path/filepath"

	"minicontainer/internal/image"
)

func applyVerifiedLayers(layerFiles []string, destDir string) (retErr error) {
	if destDir == "" {
		return fmt.Errorf("pull destination is empty")
	}
	if info, err := os.Lstat(destDir); err == nil {
		if !info.IsDir() {
			return fmt.Errorf("pull destination %q is not a directory", destDir)
		}
		return applyVerifiedLayersInPlace(layerFiles, destDir)
	} else if !os.IsNotExist(err) {
		return fmt.Errorf("inspect pull destination %q: %w", destDir, err)
	}

	destAbs, err := filepath.Abs(destDir)
	if err != nil {
		return fmt.Errorf("resolve pull destination %q: %w", destDir, err)
	}
	parent := filepath.Dir(destAbs)
	if err := os.MkdirAll(parent, 0o755); err != nil {
		return fmt.Errorf("create pull destination parent %q: %w", parent, err)
	}

	staging, err := os.MkdirTemp(parent, "."+filepath.Base(destAbs)+".pull-*")
	if err != nil {
		return fmt.Errorf("create staged pull destination: %w", err)
	}
	published := false
	defer func() {
		if published {
			return
		}
		if err := os.RemoveAll(staging); err != nil {
			retErr = errors.Join(retErr, fmt.Errorf("remove staged pull destination %q: %w", staging, err))
		}
	}()

	if err := applyVerifiedLayersInPlace(layerFiles, staging); err != nil {
		return err
	}
	if err := image.PublishDirectoryNoReplace(staging, destAbs); err != nil {
		return fmt.Errorf("publish pulled rootfs: %w", err)
	}
	published = true
	return nil
}

func applyVerifiedLayersInPlace(layerFiles []string, destDir string) error {
	for i, layerFile := range layerFiles {
		if err := image.Unpack(layerFile, destDir); err != nil {
			return fmt.Errorf("apply layer %d: %w", i+1, err)
		}
	}
	return nil
}
