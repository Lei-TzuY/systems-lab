package imagestore

import (
	"crypto/sha256"
	"encoding/hex"
	"fmt"
	"os"
	"path/filepath"

	"minicontainer/internal/state"
)

// rawRootFSContentID returns the complete SHA-256 identity of an imported raw
// rootfs archive. Storage ownership must never be derived from the historical
// 12-hex display prefix: two different archives sharing that 48-bit prefix must
// remain independent payloads with independent metadata identities.
func rawRootFSContentID(sum []byte) (string, error) {
	if len(sum) != sha256.Size {
		return "", fmt.Errorf("raw rootfs SHA-256 length is %d, want %d", len(sum), sha256.Size)
	}
	return hex.EncodeToString(sum), nil
}

// rawRootFSPayloadHasCommittedReference reports whether durable image metadata
// contains at least one exact full-content-ID reference to durableRootFS. If
// the same ID is committed with a different rootfs, state is inconsistent and
// the caller must fail closed rather than infer payload ownership.
func rawRootFSPayloadHasCommittedReference(st *state.Store, durableRootFS, contentID string) (bool, error) {
	images, err := st.ListImages()
	if err != nil {
		return false, fmt.Errorf("read image metadata ownership proof: %w", err)
	}
	wantRootFS := filepath.Clean(durableRootFS)
	found := false
	for _, img := range images {
		if img == nil || img.ID != contentID {
			continue
		}
		if filepath.Clean(img.RootFS) != wantRootFS {
			return false, fmt.Errorf(
				"image ID %s has committed metadata for unexpected rootfs %q, want %q",
				contentID,
				img.RootFS,
				durableRootFS,
			)
		}
		found = true
	}
	return found, nil
}

// verifyReusableRawRootFSPayload proves that an already-present full-digest
// payload directory belongs to a previously committed image record. Pathname
// existence alone is not ownership proof: a stale, foreign, or interrupted
// publication must never be adopted as an identical image merely because it
// occupies the expected content-addressed name.
func verifyReusableRawRootFSPayload(st *state.Store, payloadDir, durableRootFS, contentID string) error {
	info, err := os.Lstat(payloadDir)
	if err != nil {
		return fmt.Errorf("inspect existing image payload: %w", err)
	}
	if info.Mode()&os.ModeSymlink != 0 || !info.IsDir() {
		return fmt.Errorf("existing image payload %q must be a real directory", payloadDir)
	}

	found, err := rawRootFSPayloadHasCommittedReference(st, durableRootFS, contentID)
	if err != nil {
		return err
	}
	if !found {
		return fmt.Errorf("existing image payload %s has no committed metadata ownership proof", contentID)
	}
	return nil
}
