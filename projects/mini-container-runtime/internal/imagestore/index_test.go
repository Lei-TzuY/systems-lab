package imagestore

import (
	"testing"
)

func TestResolveManifestIndex(t *testing.T) {
	indexJSON := []byte(`{
		"schemaVersion": 2,
		"mediaType": "application/vnd.oci.image.index.v1+json",
		"manifests": [
			{
				"mediaType": "application/vnd.oci.image.manifest.v1+json",
				"digest": "sha256:amd64digest",
				"size": 1234,
				"platform": { "architecture": "amd64", "os": "linux" }
			},
			{
				"mediaType": "application/vnd.oci.image.manifest.v1+json",
				"digest": "sha256:arm64digest",
				"size": 5678,
				"platform": { "architecture": "arm64", "os": "linux" }
			}
		]
	}`)

	digest, err := ResolveManifestIndex(indexJSON, "linux", "arm64")
	if err != nil || digest != "sha256:arm64digest" {
		t.Fatalf("ResolveManifestIndex error: %v (digest=%s)", err, digest)
	}
}
