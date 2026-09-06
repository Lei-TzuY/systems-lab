package imagestore

import (
	"testing"
)

func TestExtractManifestSubject(t *testing.T) {
	manifestJSON := []byte(`{
		"schemaVersion": 2,
		"subject": {
			"mediaType": "application/vnd.oci.image.manifest.v1+json",
			"digest": "sha256:targetdigest",
			"size": 1234
		}
	}`)

	subj, err := ExtractManifestSubject(manifestJSON)
	if err != nil || subj == nil || subj.Digest != "sha256:targetdigest" {
		t.Fatalf("ExtractManifestSubject error: %v (subj=%v)", err, subj)
	}
}
