package state

import (
	"errors"
	"fmt"
	"os"
	"path/filepath"
)

// ImageStorageLease holds independent directory handles for the exact state
// root/images generation observed by Store.Open. The lease remains valid if the
// Store itself is closed while a payload transaction is in flight.
type ImageStorageLease struct {
	rootFile         *os.File
	imageFile        *os.File
	pinnedImagePath  string
	configuredRoot   string
	configuredImages string
}

// AcquireImageStorage leases the pinned image payload directory. Callers must
// Close the lease. The configured state pathname is verified both here and by
// callers again immediately before publishing metadata that contains durable
// configured paths.
func (s *Store) AcquireImageStorage() (*ImageStorageLease, error) {
	if s == nil {
		return nil, fmt.Errorf("state store is nil")
	}

	s.mu.Lock()
	defer s.mu.Unlock()
	if s.lockFile == nil {
		return nil, ErrStoreClosed
	}

	lease, err := acquireImageStorageLeaseLocked(s)
	if err != nil {
		return nil, err
	}
	if err := lease.ValidateConfiguredGeneration(); err != nil {
		_ = lease.Close()
		return nil, err
	}
	return lease, nil
}

// Path returns the stable path used for filesystem mutations during the lease.
func (l *ImageStorageLease) Path() string {
	if l == nil {
		return ""
	}
	return l.pinnedImagePath
}

// DurablePath returns the configured image directory pathname suitable for
// persisted RootFS metadata after ValidateConfiguredGeneration succeeds.
func (l *ImageStorageLease) DurablePath() string {
	if l == nil {
		return ""
	}
	return l.configuredImages
}

// ValidateConfiguredGeneration proves that the configured state root and image
// directory still resolve to the exact directories held by this lease. Symlink
// replacements fail closed even when they happen to lead back to the same inode.
func (l *ImageStorageLease) ValidateConfiguredGeneration() error {
	if l == nil || l.rootFile == nil || l.imageFile == nil {
		return fmt.Errorf("image storage lease is closed")
	}

	configuredRoot, err := realDirectoryInfo(l.configuredRoot, "configured state root")
	if err != nil {
		return err
	}
	configuredImages, err := realDirectoryInfo(l.configuredImages, "configured image directory")
	if err != nil {
		return err
	}
	pinnedRoot, err := l.rootFile.Stat()
	if err != nil {
		return fmt.Errorf("inspect leased state root: %w", err)
	}
	pinnedImages, err := l.imageFile.Stat()
	if err != nil {
		return fmt.Errorf("inspect leased image directory: %w", err)
	}
	if !os.SameFile(configuredRoot, pinnedRoot) {
		return fmt.Errorf("configured state root changed generation after Store.Open")
	}
	if !os.SameFile(configuredImages, pinnedImages) {
		return fmt.Errorf("configured image directory changed generation after Store.Open")
	}
	return nil
}

func realDirectoryInfo(path, label string) (os.FileInfo, error) {
	info, err := os.Lstat(path)
	if err != nil {
		return nil, fmt.Errorf("inspect %s %q: %w", label, path, err)
	}
	if info.Mode()&os.ModeSymlink != 0 || !info.IsDir() {
		return nil, fmt.Errorf("%s %q must be a real directory", label, path)
	}
	return info, nil
}

// Close releases the independent lease handles. It is safe to call repeatedly.
func (l *ImageStorageLease) Close() error {
	if l == nil {
		return nil
	}
	var errs []error
	if l.imageFile != nil {
		if err := l.imageFile.Close(); err != nil {
			errs = append(errs, fmt.Errorf("close leased image directory: %w", err))
		}
		l.imageFile = nil
	}
	if l.rootFile != nil {
		if err := l.rootFile.Close(); err != nil {
			errs = append(errs, fmt.Errorf("close leased state root: %w", err))
		}
		l.rootFile = nil
	}
	l.pinnedImagePath = ""
	return errors.Join(errs...)
}

func newImageStorageLease(rootFile, imageFile *os.File, pinnedImagePath, configuredRoot string) *ImageStorageLease {
	return &ImageStorageLease{
		rootFile:         rootFile,
		imageFile:        imageFile,
		pinnedImagePath:  pinnedImagePath,
		configuredRoot:   configuredRoot,
		configuredImages: filepath.Join(configuredRoot, "images"),
	}
}
