//go:build !linux

package imagestore

import (
	"fmt"
	"os"
	"path/filepath"
)

type portableManagedImageRootFSRemoval struct {
	path    string
	absent  bool
	removed bool
}

func pinManagedImageRootFS(imagesPath, imageID string) (managedImageRootFSRemoval, error) {
	rootFS := filepath.Join(imagesPath, imageID, "rootfs")
	info, err := os.Lstat(rootFS)
	if err != nil {
		if os.IsNotExist(err) {
			return &portableManagedImageRootFSRemoval{path: rootFS, absent: true}, nil
		}
		return nil, fmt.Errorf("inspect managed image rootfs for %q: %w", imageID, err)
	}
	if info.Mode()&os.ModeSymlink != 0 || !info.IsDir() {
		return nil, fmt.Errorf("managed image rootfs for %q must be a real directory", imageID)
	}
	return &portableManagedImageRootFSRemoval{path: rootFS}, nil
}

func (r *portableManagedImageRootFSRemoval) Remove() error {
	if r == nil || r.absent || r.removed {
		return nil
	}
	info, err := os.Lstat(r.path)
	if err != nil {
		if os.IsNotExist(err) {
			r.removed = true
			return nil
		}
		return fmt.Errorf("recheck managed image rootfs: %w", err)
	}
	if info.Mode()&os.ModeSymlink != 0 || !info.IsDir() {
		return fmt.Errorf("managed image rootfs changed type before removal")
	}
	if err := os.RemoveAll(r.path); err != nil {
		return fmt.Errorf("remove managed image rootfs: %w", err)
	}
	r.removed = true
	return nil
}

func (r *portableManagedImageRootFSRemoval) Close() error { return nil }
