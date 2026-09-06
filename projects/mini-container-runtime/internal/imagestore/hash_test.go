package imagestore

import (
	"strings"
	"testing"
)

func TestComputeManifestDigest(t *testing.T) {
	manifest := []byte(`{"schemaVersion":2}`)
	digest := ComputeManifestDigest(manifest)
	if !strings.HasPrefix(digest, "sha256:") || len(digest) != 71 {
		t.Fatalf("ComputeManifestDigest = %s, want sha256:hash", digest)
	}
}
