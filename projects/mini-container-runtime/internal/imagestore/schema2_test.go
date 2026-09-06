package imagestore

import (
	"testing"
)

func TestValidateSchema2Manifest(t *testing.T) {
	manifestJSON := []byte(`{
		"schemaVersion": 2,
		"mediaType": "application/vnd.docker.distribution.manifest.v2+json",
		"config": {
			"mediaType": "application/vnd.docker.container.image.v1+json",
			"size": 7023,
			"digest": "sha256:b5b15809"
		},
		"layers": []
	}`)

	valid, err := ValidateSchema2Manifest(manifestJSON)
	if err != nil || !valid {
		t.Fatalf("ValidateSchema2Manifest error: %v (valid=%v)", err, valid)
	}
}
