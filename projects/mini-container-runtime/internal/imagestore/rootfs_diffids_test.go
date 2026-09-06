package imagestore

import (
	"testing"
)

func TestExtractRootFSDiffIDs(t *testing.T) {
	configJSON := []byte(`{
		"rootfs": {
			"type": "layers",
			"diff_ids": ["sha256:d1", "sha256:d2"]
		}
	}`)

	diffs := ExtractRootFSDiffIDs(configJSON)
	if len(diffs) != 2 || diffs[0] != "sha256:d1" {
		t.Fatalf("ExtractRootFSDiffIDs = %v, want diff_ids", diffs)
	}
}
