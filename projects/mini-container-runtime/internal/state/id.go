// internal/state/id.go
//
// Container ID Generation
// ────────────────────────
// Docker uses the first 12 hex characters of the image+config SHA-256 hash.
// We generate a random 8-byte (64-bit) value — long enough to be unique
// in any realistic local-machine scenario, short enough to type by hand.
//
// The full 16-character ID is what we store on disk.  Users can refer to
// a container by any unambiguous prefix (e.g., "a3f8" → "a3f8b2c1d0e4f6a2").

package state

import (
	"crypto/rand"
	"encoding/hex"
	"fmt"
)

// NewID generates a random 16-character hex container ID.
func NewID() (string, error) {
	b := make([]byte, 8)
	if _, err := rand.Read(b); err != nil {
		return "", fmt.Errorf("generate container ID: %w", err)
	}
	return hex.EncodeToString(b), nil
}
