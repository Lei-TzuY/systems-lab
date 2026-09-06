package imagestore

import (
	"strings"
	"testing"
)

func TestBuildManifestAcceptHeader(t *testing.T) {
	header := BuildManifestAcceptHeader()
	if !strings.Contains(header, "application/vnd.oci.image.manifest.v1+json") ||
		!strings.Contains(header, "application/vnd.docker.distribution.manifest.v2+json") {
		t.Fatalf("BuildManifestAcceptHeader = %s, want complete Accept header", header)
	}
}
