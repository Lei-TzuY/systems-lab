package imagestore

import (
	"crypto/sha256"
	"fmt"
)

// CalculateConfigDigest computes sha256 canonical digest string of image config JSON.
func CalculateConfigDigest(configJSON []byte) string {
	hash := sha256.Sum256(configJSON)
	return fmt.Sprintf("sha256:%x", hash)
}
