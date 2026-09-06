package imagestore

import (
	"testing"
)

func TestCalculateManifestTotalSize(t *testing.T) {
	manifestJSON := []byte(`{
		"config": { "size": 1000 },
		"layers": [
			{ "size": 2000 },
			{ "size": 3000 }
		]
	}`)

	total, err := CalculateManifestTotalSize(manifestJSON)
	if err != nil || total != 6000 {
		t.Fatalf("CalculateManifestTotalSize error: %v (total=%d, want 6000)", err, total)
	}
}
