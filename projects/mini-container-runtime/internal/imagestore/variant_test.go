package imagestore

import (
	"testing"
)

func TestExtractImageVariant(t *testing.T) {
	configJSON := []byte(`{
		"architecture": "arm",
		"variant": "v7"
	}`)

	variant := ExtractImageVariant(configJSON)
	if variant != "v7" {
		t.Fatalf("ExtractImageVariant = %s, want v7", variant)
	}
}
