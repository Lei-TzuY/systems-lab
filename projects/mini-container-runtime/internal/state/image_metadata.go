package state

import (
	"crypto/sha256"
	"encoding/hex"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"reflect"
	"strings"
)

const maxLegacyImageMetadataFilenameBytes = 255

func imageStorageKey(img *Image) (string, error) {
	if img == nil {
		return "", fmt.Errorf("image state is nil")
	}
	if img.Name != "" {
		if err := validateImageSelector(img.Name); err != nil {
			return "", fmt.Errorf("invalid image name: %w", err)
		}
	}
	if img.ID != "" {
		if err := validateImageSelector(img.ID); err != nil {
			return "", fmt.Errorf("invalid image ID: %w", err)
		}
	}
	key := img.Name
	if key == "" {
		key = img.ID
	}
	if err := validateImageSelector(key); err != nil {
		return "", err
	}
	return key, nil
}

func validateImageRootFS(rootfs string) error {
	if rootfs == "" {
		return nil
	}
	if strings.TrimSpace(rootfs) == "" {
		return fmt.Errorf("image rootfs cannot be whitespace-only")
	}
	if strings.IndexByte(rootfs, 0) >= 0 {
		return fmt.Errorf("image rootfs contains NUL byte")
	}
	if !filepath.IsAbs(rootfs) {
		return fmt.Errorf("image rootfs %q must be absolute", rootfs)
	}
	if clean := filepath.Clean(rootfs); clean != rootfs {
		return fmt.Errorf("image rootfs %q must be clean (canonical lexical path %q)", rootfs, clean)
	}
	return nil
}

func imageMetadataFilename(key string) string {
	sum := sha256.Sum256([]byte(key))
	return "img-" + hex.EncodeToString(sum[:]) + ".json"
}

func legacyImageMetadataFilename(key string) string {
	return sanitizeImageFilename(key) + ".json"
}

func legacyImageMetadataPath(dir, key string) (string, bool) {
	limit, ok := imageMetadataComponentLimit(dir)
	if !ok {
		return "", false
	}
	name := legacyImageMetadataFilename(key)
	if len(name) > limit {
		return "", false
	}
	return filepath.Join(dir, name), true
}

func validateImageMetadataPath(path, key string) error {
	base := filepath.Base(path)
	if base == imageMetadataFilename(key) || base == legacyImageMetadataFilename(key) {
		return nil
	}
	return fmt.Errorf("image metadata pathname %q does not match storage key %q", base, key)
}

func readImageMetadata(path string) (*Image, error) {
	data, err := readRegularStateFile(path, "image state")
	if err != nil {
		return nil, err
	}
	var img Image
	if err := decodeImageMetadataStrict(data, &img); err != nil {
		return nil, fmt.Errorf("unmarshal image state %q: %w", filepath.Base(path), err)
	}
	key, err := imageStorageKey(&img)
	if err != nil {
		return nil, fmt.Errorf("invalid image state %q: %w", filepath.Base(path), err)
	}
	if err := validateImageMetadataPath(path, key); err != nil {
		return nil, err
	}
	return &img, nil
}

func imageMetadataOwnedBy(path, key string) (bool, error) {
	img, err := readImageMetadata(path)
	if err != nil {
		if errors.Is(err, os.ErrNotExist) {
			return false, nil
		}
		return false, err
	}
	storedKey, err := imageStorageKey(img)
	if err != nil {
		return false, err
	}
	return storedKey == key, nil
}

// validateImagePublicationAgainstRegistry checks every registry-wide invariant
// against one authoritative snapshot. Keeping the checks in one pass prevents
// SaveImage from re-reading the image directory between related decisions and
// ensures all publication constraints are evaluated against the same generation.
func validateImagePublicationAgainstRegistry(images []*Image, img *Image, key string) error {
	for _, existing := range images {
		existingKey, err := imageStorageKey(existing)
		if err != nil {
			return fmt.Errorf("inspect existing image metadata: %w", err)
		}
		if existingKey == key {
			continue
		}
		if img.Name != "" && img.Name == existing.ID {
			return fmt.Errorf("ambiguous image identity publication: image name %q collides with exact ID of %q", img.Name, existingKey)
		}
		if img.ID != "" && img.ID == existing.Name {
			return fmt.Errorf("ambiguous image identity publication: image ID %q collides with exact name of %q", img.ID, existingKey)
		}
		if img.ID != "" && existing.ID == img.ID && existing.RootFS != img.RootFS {
			return fmt.Errorf("inconsistent image alias publication: image ID %s already references rootfs %q via %q, cannot publish alias %q with rootfs %q", img.ID, existing.RootFS, existingKey, key, img.RootFS)
		}
	}
	return nil
}

func (s *Store) ensureImageRegistryConsistentUnlocked(img *Image, key string) error {
	images, err := s.listImagesUnlocked()
	if err != nil {
		return fmt.Errorf("inspect existing image registry: %w", err)
	}
	return validateImagePublicationAgainstRegistry(images, img, key)
}

func (s *Store) saveImageMetadataUnlocked(img *Image, data []byte) error {
	key, err := imageStorageKey(img)
	if err != nil {
		return err
	}
	if err := validateImageRootFS(img.RootFS); err != nil {
		return fmt.Errorf("refuse invalid image rootfs publication: %w", err)
	}
	if err := validateStateFileWrite(data, "image state"); err != nil {
		return err
	}
	if err := s.ensureImageNotPendingCleanupUnlocked(img); err != nil {
		return fmt.Errorf("refuse image metadata publication during pending cleanup: %w", err)
	}
	if err := s.ensureImageRegistryConsistentUnlocked(img, key); err != nil {
		return fmt.Errorf("refuse image registry publication: %w", err)
	}
	newPath := filepath.Join(s.imgDir, imageMetadataFilename(key))
	legacyPath, legacyPathUsable := legacyImageMetadataPath(s.imgDir, key)

	migrateLegacy := false
	if legacyPathUsable && legacyPath != newPath {
		owned, err := imageMetadataOwnedBy(legacyPath, key)
		if err != nil {
			return fmt.Errorf("inspect legacy image metadata for %q: %w", key, err)
		}
		migrateLegacy = owned
	}

	if err := atomicWriteFile(s.imgDir, newPath, data); err != nil {
		return err
	}
	if migrateLegacy {
		if err := removeStateFileDurable(s.imgDir, legacyPath, "legacy image metadata"); err != nil {
			return err
		}
	}
	return nil
}

func (s *Store) removeImageMetadataUnlocked(img *Image) error {
	key, err := imageStorageKey(img)
	if err != nil {
		return err
	}
	paths := []string{filepath.Join(s.imgDir, imageMetadataFilename(key))}
	if legacyPath, ok := legacyImageMetadataPath(s.imgDir, key); ok {
		paths = append(paths, legacyPath)
	}
	seenPaths := make(map[string]bool, len(paths))
	var errs []error
	for _, path := range paths {
		if seenPaths[path] {
			continue
		}
		seenPaths[path] = true
		owned, err := imageMetadataOwnedBy(path, key)
		if err != nil {
			errs = append(errs, fmt.Errorf("inspect image metadata %q: %w", filepath.Base(path), err))
			continue
		}
		if !owned {
			continue
		}
		if err := removeStateFileDurable(s.imgDir, path, "image metadata"); err != nil {
			errs = append(errs, err)
		}
	}
	return errors.Join(errs...)
}

type seenImageMetadata struct {
	img     *Image
	current bool
	index   int
}

func isCurrentImageMetadataPath(path, key string) bool {
	return filepath.Base(path) == imageMetadataFilename(key)
}

func appendUniqueImageMetadata(out []*Image, seen map[string]seenImageMetadata, img *Image, path string) ([]*Image, error) {
	key, err := imageStorageKey(img)
	if err != nil {
		return nil, err
	}
	current := isCurrentImageMetadataPath(path, key)
	previous, ok := seen[key]
	if !ok {
		seen[key] = seenImageMetadata{img: img, current: current, index: len(out)}
		return append(out, img), nil
	}

	switch {
	case previous.current && current:
		if !reflect.DeepEqual(previous.img, img) {
			return nil, fmt.Errorf("conflicting current image metadata for %q", key)
		}
		return out, nil
	case previous.current && !current:
		return out, nil
	case !previous.current && current:
		out[previous.index] = img
		seen[key] = seenImageMetadata{img: img, current: true, index: previous.index}
		return out, nil
	default:
		if !reflect.DeepEqual(previous.img, img) {
			return nil, fmt.Errorf("conflicting duplicate image metadata for %q", key)
		}
		return out, nil
	}
}
