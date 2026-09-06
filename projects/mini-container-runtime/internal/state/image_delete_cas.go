package state

import (
	"fmt"
	"reflect"
)

// DeleteImageIfMatch removes the image selected by nameOrID only when its
// current durable metadata still exactly matches expected. Destructive callers
// can therefore validate and pin resources from a snapshot without later
// deleting a different image record that replaced the selector in the gap.
func (s *Store) DeleteImageIfMatch(nameOrID string, expected *Image) (*Image, error) {
	if err := validateImageSelector(nameOrID); err != nil {
		return nil, err
	}
	if expected == nil {
		return nil, fmt.Errorf("expected image state is nil")
	}

	s.mu.Lock()
	defer s.mu.Unlock()
	if err := lockStateFile(s.lockFile); err != nil {
		return nil, err
	}
	defer func() { _ = unlockStateFile(s.lockFile) }()

	images, err := s.listImagesUnlocked()
	if err != nil {
		return nil, err
	}
	current, err := resolveImageForDelete(images, nameOrID)
	if err != nil {
		return nil, err
	}
	if !reflect.DeepEqual(current, expected) {
		return nil, fmt.Errorf("image %q changed after destructive preflight", nameOrID)
	}
	if err := s.removeImageMetadataUnlocked(current); err != nil {
		return nil, err
	}
	return current, nil
}
