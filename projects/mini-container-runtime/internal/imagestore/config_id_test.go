package imagestore

import (
	"strings"
	"testing"
)

func TestCalculateConfigDigest(t *testing.T) {
	digest := CalculateConfigDigest([]byte(`{"architecture":"amd64"}`))
	if !strings.HasPrefix(digest, "sha256:") || len(digest) != 71 {
		t.Fatalf("CalculateConfigDigest = %s, want sha256 digest string", digest)
	}
}
