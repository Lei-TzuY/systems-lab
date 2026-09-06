package state

import (
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"reflect"
	"strings"
)

const imageCleanupSuffix = ".image-cleanup"

// ImageCleanup is a durable proof that one Store-managed image payload may be
// removed after its last metadata reference is deleted. The proof is written
// before metadata mutation and is cleared only after payload cleanup succeeds.
type ImageCleanup struct {
	ID     string `json:"id"`
	RootFS string `json:"rootfs"`
}

func normalizeImageCleanup(cleanup ImageCleanup) (ImageCleanup, error) {
	if err := validateImageSelector(cleanup.ID); err != nil {
		return ImageCleanup{}, fmt.Errorf("invalid image cleanup ID: %w", err)
	}
	if cleanup.RootFS == "" {
		return ImageCleanup{}, fmt.Errorf("image cleanup rootfs is empty")
	}
	cleanup.RootFS = filepath.Clean(cleanup.RootFS)
	if !filepath.IsAbs(cleanup.RootFS) {
		return ImageCleanup{}, fmt.Errorf("image cleanup rootfs %q is not absolute", cleanup.RootFS)
	}
	return cleanup, nil
}

func imageCleanupFilename(cleanup ImageCleanup) string {
	sum := sha256.Sum256([]byte(cleanup.ID + "\x00" + cleanup.RootFS))
	return "cleanup-" + hex.EncodeToString(sum[:]) + imageCleanupSuffix
}

func imageCleanupPath(imageDir string, cleanup ImageCleanup) string {
	return filepath.Join(imageDir, imageCleanupFilename(cleanup))
}

func readImageCleanup(path string) (ImageCleanup, error) {
	data, err := readRegularStateFile(path, "image cleanup ownership")
	if err != nil {
		return ImageCleanup{}, err
	}
	var cleanup ImageCleanup
	if err := json.Unmarshal(data, &cleanup); err != nil {
		return ImageCleanup{}, fmt.Errorf("unmarshal image cleanup %q: %w", filepath.Base(path), err)
	}
	cleanup, err = normalizeImageCleanup(cleanup)
	if err != nil {
		return ImageCleanup{}, fmt.Errorf("invalid image cleanup %q: %w", filepath.Base(path), err)
	}
	if filepath.Base(path) != imageCleanupFilename(cleanup) {
		return ImageCleanup{}, fmt.Errorf("image cleanup %q does not match its durable identity", filepath.Base(path))
	}
	return cleanup, nil
}

func (s *Store) listImageCleanupsUnlocked() ([]ImageCleanup, error) {
	entries, err := os.ReadDir(s.imgDir)
	if err != nil {
		return nil, fmt.Errorf("read image cleanup directory: %w", err)
	}
	cleanups := make([]ImageCleanup, 0)
	for _, entry := range entries {
		if !strings.HasSuffix(entry.Name(), imageCleanupSuffix) {
			continue
		}
		if entry.IsDir() {
			return nil, fmt.Errorf("image cleanup %q is not a regular file", entry.Name())
		}
		cleanup, err := readImageCleanup(filepath.Join(s.imgDir, entry.Name()))
		if err != nil {
			return nil, err
		}
		cleanups = append(cleanups, cleanup)
	}
	return cleanups, nil
}

func (s *Store) writeImageCleanupUnlocked(cleanup ImageCleanup) error {
	cleanup, err := normalizeImageCleanup(cleanup)
	if err != nil {
		return err
	}
	existing, err := s.listImageCleanupsUnlocked()
	if err != nil {
		return err
	}
	for _, pending := range existing {
		if pending == cleanup {
			return nil
		}
		if pending.ID == cleanup.ID || filepath.Clean(pending.RootFS) == cleanup.RootFS {
			return fmt.Errorf("conflicting pending image cleanup for ID %q or rootfs %q", cleanup.ID, cleanup.RootFS)
		}
	}
	data, err := json.Marshal(cleanup)
	if err != nil {
		return fmt.Errorf("marshal image cleanup: %w", err)
	}
	return atomicWriteFile(s.imgDir, imageCleanupPath(s.imgDir, cleanup), data)
}

func (s *Store) clearImageCleanupUnlocked(cleanup ImageCleanup) (bool, error) {
	cleanup, err := normalizeImageCleanup(cleanup)
	if err != nil {
		return false, err
	}
	path := imageCleanupPath(s.imgDir, cleanup)
	persisted, err := readImageCleanup(path)
	if err != nil {
		if errors.Is(err, os.ErrNotExist) {
			return false, nil
		}
		return false, err
	}
	if persisted != cleanup {
		return false, fmt.Errorf("pending image cleanup changed before clear")
	}
	if err := removeStateFileDurable(s.imgDir, path, "image cleanup ownership"); err != nil {
		return false, err
	}
	return true, nil
}

func (s *Store) ensureImageNotPendingCleanupUnlocked(img *Image) error {
	if img == nil {
		return fmt.Errorf("image state is nil")
	}
	cleanups, err := s.listImageCleanupsUnlocked()
	if err != nil {
		return err
	}
	rootFS := filepath.Clean(img.RootFS)
	for _, cleanup := range cleanups {
		if cleanup.ID == img.ID || (img.RootFS != "" && filepath.Clean(cleanup.RootFS) == rootFS) {
			return fmt.Errorf("image ID %q or rootfs %q has pending cleanup ownership", img.ID, img.RootFS)
		}
	}
	return nil
}

// ListImageCleanups returns all durable managed-image cleanup proofs. A caller
// may use these records to recover deletion after a process crash.
func (s *Store) ListImageCleanups() ([]ImageCleanup, error) {
	if s == nil {
		return nil, fmt.Errorf("state store is nil")
	}
	s.mu.Lock()
	defer s.mu.Unlock()
	if s.lockFile == nil {
		return nil, ErrStoreClosed
	}
	return s.listImageCleanupsUnlocked()
}

// ClearImageCleanupIfMatch clears one exact durable cleanup proof after payload
// removal succeeds or recovery proves that live metadata still owns the rootfs.
func (s *Store) ClearImageCleanupIfMatch(cleanup ImageCleanup) (bool, error) {
	if s == nil {
		return false, fmt.Errorf("state store is nil")
	}
	cleanup, err := normalizeImageCleanup(cleanup)
	if err != nil {
		return false, err
	}
	s.mu.Lock()
	defer s.mu.Unlock()
	if err := lockStateFile(s.lockFile); err != nil {
		return false, err
	}
	defer func() { _ = unlockStateFile(s.lockFile) }()
	return s.clearImageCleanupUnlocked(cleanup)
}

// DeleteImageIfMatchWithCleanup conditionally deletes one exact metadata record
// and, when it was the final rootfs reference, durably arms cleanup first. The
// sidecar survives any subsequent metadata-removal or payload-removal failure.
func (s *Store) DeleteImageIfMatchWithCleanup(nameOrID string, expected *Image, cleanup ImageCleanup) (*Image, bool, error) {
	if err := validateImageSelector(nameOrID); err != nil {
		return nil, false, err
	}
	if expected == nil {
		return nil, false, fmt.Errorf("expected image state is nil")
	}
	cleanup, err := normalizeImageCleanup(cleanup)
	if err != nil {
		return nil, false, err
	}

	s.mu.Lock()
	defer s.mu.Unlock()
	if err := lockStateFile(s.lockFile); err != nil {
		return nil, false, err
	}
	defer func() { _ = unlockStateFile(s.lockFile) }()

	images, err := s.listImagesUnlocked()
	if err != nil {
		return nil, false, err
	}
	current, err := resolveImageForDelete(images, nameOrID)
	if err != nil {
		return nil, false, err
	}
	if !reflect.DeepEqual(current, expected) {
		return nil, false, fmt.Errorf("image %q changed after destructive preflight", nameOrID)
	}
	if current.ID != cleanup.ID || filepath.Clean(current.RootFS) != cleanup.RootFS {
		return nil, false, fmt.Errorf("image cleanup ownership does not match selected image")
	}

	for _, other := range images {
		if other == nil || other == current {
			continue
		}
		otherRootFS := filepath.Clean(other.RootFS)
		if other.ID == current.ID && otherRootFS != cleanup.RootFS {
			return nil, false, fmt.Errorf("inconsistent image aliases for ID %s reference rootfs %q and %q", current.ID, cleanup.RootFS, otherRootFS)
		}
		if otherRootFS == cleanup.RootFS {
			if err := s.removeImageMetadataUnlocked(current); err != nil {
				return nil, false, err
			}
			return current, false, nil
		}
	}

	if err := s.writeImageCleanupUnlocked(cleanup); err != nil {
		return nil, false, fmt.Errorf("persist image cleanup ownership: %w", err)
	}
	if err := s.removeImageMetadataUnlocked(current); err != nil {
		// Do not clear the sidecar here. Durable unlink may already have removed
		// metadata before reporting a directory-sync failure; the sidecar is the
		// only crash-safe authority that lets recovery distinguish that case.
		return nil, true, fmt.Errorf("remove image metadata after arming cleanup: %w", err)
	}
	return current, true, nil
}
