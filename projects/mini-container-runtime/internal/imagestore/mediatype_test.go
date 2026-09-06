package imagestore

import (
	"testing"
)

func TestValidateMediaType(t *testing.T) {
	if !ValidateMediaType(MediaTypeOCIManifest) {
		t.Fatalf("ValidateMediaType(%s) = false, want true", MediaTypeOCIManifest)
	}
	if ValidateMediaType("text/plain") {
		t.Fatalf("ValidateMediaType(text/plain) = true, want false")
	}
}
