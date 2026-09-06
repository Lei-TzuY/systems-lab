package imagestore

import (
	"testing"
)

func TestParseManifestAnnotations(t *testing.T) {
	manifestJSON := []byte(`{
		"schemaVersion": 2,
		"annotations": {
			"org.opencontainers.image.title": "minictl-app",
			"org.opencontainers.image.version": "1.0.0"
		}
	}`)

	annos, err := ParseManifestAnnotations(manifestJSON)
	if err != nil || annos["org.opencontainers.image.title"] != "minictl-app" {
		t.Fatalf("ParseManifestAnnotations error: %v (annos=%v)", err, annos)
	}
}
