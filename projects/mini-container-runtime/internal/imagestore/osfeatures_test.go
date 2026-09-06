package imagestore

import (
	"testing"
)

func TestExtractOSFeatures(t *testing.T) {
	configJSON := []byte(`{
		"os.features": ["win32k", "hyperv"]
	}`)

	feats := ExtractOSFeatures(configJSON)
	if len(feats) != 2 || feats[0] != "win32k" {
		t.Fatalf("ExtractOSFeatures = %v, want [win32k hyperv]", feats)
	}
}
