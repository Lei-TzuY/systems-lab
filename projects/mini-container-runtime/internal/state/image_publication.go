package state

import (
	"encoding/json"
	"fmt"
	"path/filepath"
	"reflect"
)

func sameImagePayload(a, b *Image) bool {
	if a == nil || b == nil {
		return false
	}
	return a.ID == b.ID && filepath.Clean(a.RootFS) == filepath.Clean(b.RootFS)
}

func imageByStorageKey(images []*Image, key string) (*Image, error) {
	var match *Image
	for _, img := range images {
		if img == nil {
			continue
		}
		candidateKey, err := imageStorageKey(img)
		if err != nil {
			return nil, err
		}
		if candidateKey != key {
			continue
		}
		if match != nil {
			return nil, fmt.Errorf("multiple image records use storage key %q", key)
		}
		match = img
	}
	return match, nil
}

func validatePublishedPayloadAliases(images []*Image, published *Image) error {
	if published == nil || published.ID == "" {
		return nil
	}
	rootFS := filepath.Clean(published.RootFS)
	for _, other := range images {
		if other == nil || other.ID != published.ID {
			continue
		}
		otherRootFS := filepath.Clean(other.RootFS)
		if otherRootFS != rootFS {
			return fmt.Errorf(
				"refusing to publish image ID %s with rootfs %q: existing alias references %q",
				published.ID,
				rootFS,
				otherRootFS,
			)
		}
	}
	return nil
}

func validateExistingPayloadAliases(images []*Image, existing *Image) (bool, error) {
	if existing == nil {
		return false, nil
	}
	rootFS := filepath.Clean(existing.RootFS)
	hasOtherReference := false
	for _, other := range images {
		if other == nil || other == existing {
			continue
		}
		otherRootFS := filepath.Clean(other.RootFS)
		if existing.ID != "" && other.ID == existing.ID && otherRootFS != rootFS {
			return false, fmt.Errorf("inconsistent image aliases for ID %s reference rootfs %q and %q", existing.ID, rootFS, otherRootFS)
		}
		if otherRootFS == rootFS {
			hasOtherReference = true
		}
	}
	return hasOtherReference, nil
}

func (s *Store) publishImageUnlocked(images []*Image, img *Image) error {
	key, err := imageStorageKey(img)
	if err != nil {
		return err
	}
	if err := validatePublishedPayloadAliases(images, img); err != nil {
		return err
	}
	existing, err := imageByStorageKey(images, key)
	if err != nil {
		return err
	}

	if existing != nil && !sameImagePayload(existing, img) {
		existingRootFS := filepath.Clean(existing.RootFS)
		newRootFS := filepath.Clean(img.RootFS)
		if existing.ID != "" && existing.ID == img.ID {
			return fmt.Errorf("refusing to replace image key %q: ID %s changed rootfs from %q to %q", key, existing.ID, existingRootFS, newRootFS)
		}
		if existing.RootFS != "" && img.RootFS != "" && existingRootFS == newRootFS && existing.ID != img.ID {
			return fmt.Errorf("refusing to replace image key %q: rootfs %q changed image ID from %q to %q", key, existingRootFS, existing.ID, img.ID)
		}

		hasOtherReference, err := validateExistingPayloadAliases(images, existing)
		if err != nil {
			return err
		}
		if !hasOtherReference && existing.RootFS != "" {
			if existing.ID == "" {
				return fmt.Errorf("refusing to replace image key %q: displaced payload has no image ID for durable dangling ownership", key)
			}
			if key == existing.ID {
				return fmt.Errorf("refusing to replace image key %q: displaced payload ID collides with its dangling storage key", key)
			}

			danglingKey := existing.ID
			already, err := imageByStorageKey(images, danglingKey)
			if err != nil {
				return err
			}
			if already != nil {
				if !sameImagePayload(already, existing) {
					return fmt.Errorf("cannot preserve displaced payload %s: dangling key %q belongs to another image", existing.ID, danglingKey)
				}
			} else {
				dangling := *existing
				dangling.Name = ""
				dangling.Repository = ""
				dangling.Tag = "<none>"
				data, err := json.MarshalIndent(&dangling, "", "  ")
				if err != nil {
					return fmt.Errorf("marshal displaced image dangling metadata: %w", err)
				}
				if err := s.saveImageMetadataUnlocked(&dangling, data); err != nil {
					return fmt.Errorf("preserve displaced image payload as dangling metadata: %w", err)
				}
			}
		}
	}

	data, err := json.MarshalIndent(img, "", "  ")
	if err != nil {
		return fmt.Errorf("marshal image: %w", err)
	}
	return s.saveImageMetadataUnlocked(img, data)
}

// PublishImage saves image metadata while preserving the payload displaced by a
// tag/key replacement. Ordinary metadata updates for the same ID/rootfs remain
// in-place. When one logical key moves from payload A to payload B and A has no
// other metadata reference, A is first durably converted into a dangling image
// record keyed by its ID. Only then is the requested key overwritten.
//
// This ordering is crash-safe: a failure after the dangling write leaves an
// extra safe reference, while a successful overwrite never makes the old
// payload unreachable to prune. Callers that merely update fields on the same
// payload may continue using SaveImage directly.
func (s *Store) PublishImage(img *Image) error {
	if s == nil {
		return fmt.Errorf("state store is nil")
	}
	if _, err := imageStorageKey(img); err != nil {
		return err
	}

	s.mu.Lock()
	defer s.mu.Unlock()
	if err := lockStateFile(s.lockFile); err != nil {
		return err
	}
	defer func() { _ = unlockStateFile(s.lockFile) }()

	images, err := s.listImagesUnlocked()
	if err != nil {
		return err
	}
	return s.publishImageUnlocked(images, img)
}

// PublishImageIfSourceMatch publishes an image alias only while source still
// resolves to the exact durable snapshot the caller previously observed. The
// source proof and target publication share one process/file lock, preventing a
// stale source snapshot from resurrecting metadata after its payload was
// concurrently removed.
func (s *Store) PublishImageIfSourceMatch(source string, expectedSource, published *Image) error {
	if s == nil {
		return fmt.Errorf("state store is nil")
	}
	if err := validateImageSelector(source); err != nil {
		return err
	}
	if expectedSource == nil {
		return fmt.Errorf("expected source image is nil")
	}
	if published == nil {
		return fmt.Errorf("published image is nil")
	}
	if _, err := imageStorageKey(published); err != nil {
		return err
	}
	if !sameImagePayload(expectedSource, published) {
		return fmt.Errorf("published image does not alias the expected source payload")
	}

	s.mu.Lock()
	defer s.mu.Unlock()
	if err := lockStateFile(s.lockFile); err != nil {
		return err
	}
	defer func() { _ = unlockStateFile(s.lockFile) }()

	images, err := s.listImagesUnlocked()
	if err != nil {
		return err
	}
	current, err := resolveImageForRead(images, source)
	if err != nil {
		return fmt.Errorf("source image %q changed before tag publication: %w", source, err)
	}
	if !reflect.DeepEqual(current, expectedSource) {
		return fmt.Errorf("source image %q changed before tag publication", source)
	}
	if !sameImagePayload(current, published) {
		return fmt.Errorf("published image does not alias the current source payload")
	}
	return s.publishImageUnlocked(images, published)
}
