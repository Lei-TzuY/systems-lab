package imagestore

import (
	"strings"
	"testing"
)

func TestCorrelateManifestAndConfigLayers(t *testing.T) {
	manifestJSON := `{
		"schemaVersion": 2,
		"layers": [
			{"mediaType": "application/vnd.oci.image.layer.v1.tar+gzip", "digest": "sha256:comp1", "size": 1024},
			{"mediaType": "application/vnd.oci.image.layer.v1.tar+gzip", "digest": "sha256:comp2", "size": 2048}
		]
	}`

	configJSON := `{
		"rootfs": {
			"type": "layers",
			"diff_ids": [
				"sha256:uncomp1",
				"sha256:uncomp2"
			]
		}
	}`

	info, err := CorrelateManifestAndConfigLayers([]byte(manifestJSON), []byte(configJSON))
	if err != nil {
		t.Fatalf("CorrelateManifestAndConfigLayers failed: %v", err)
	}

	if info.LayerCount != 2 {
		t.Errorf("LayerCount = %d, want 2", info.LayerCount)
	}
	if info.TotalCompressed != 3072 {
		t.Errorf("TotalCompressed = %d, want 3072", info.TotalCompressed)
	}
	if info.Layers[0].CompressedDigest != "sha256:comp1" || info.Layers[0].UncompressedHash != "sha256:uncomp1" {
		t.Errorf("unexpected layer 0 mapping: %+v", info.Layers[0])
	}
}

func TestFormatCorrelatedLayers(t *testing.T) {
	manifestJSON := `{"layers":[{"digest":"sha256:comp11111111111111111","size":100}]}`
	configJSON := `{"rootfs":{"diff_ids":["sha256:uncomp11111111111111"]}}`

	got := FormatCorrelatedLayers([]byte(manifestJSON), []byte(configJSON))
	if !strings.Contains(got, "Correlated Layers: 1") {
		t.Errorf("expected 'Correlated Layers: 1' in %q", got)
	}
	if !strings.Contains(got, "sha256:comp11111111...") {
		t.Errorf("expected truncated digest in %q", got)
	}
}
