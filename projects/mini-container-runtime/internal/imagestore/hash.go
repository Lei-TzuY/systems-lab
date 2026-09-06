package imagestore

import (
	"crypto/sha256"
	"fmt"
)

// ComputeManifestDigest returns the sha256:hash string of raw manifest bytes.
func ComputeManifestDigest(manifestBytes []byte) string {
	sum := sha256.Sum256(manifestBytes)
	return fmt.Sprintf("sha256:%x", sum)
}
