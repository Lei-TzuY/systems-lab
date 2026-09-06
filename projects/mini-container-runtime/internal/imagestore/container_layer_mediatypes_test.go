package imagestore

import (
	"strings"
	"testing"
)

func TestExtractLayerMediaTypes(t *testing.T) {
	tests := []struct {
		name       string
		json       string
		wantLayers int
		wantBytes  int64
		wantGzip   int
		wantZstd   int
		wantErr    bool
	}{
		{
			name: "mixed gzip and zstd layers",
			json: `{
				"schemaVersion": 2,
				"layers": [
					{"mediaType": "application/vnd.oci.image.layer.v1.tar+gzip", "size": 1000},
					{"mediaType": "application/vnd.oci.image.layer.v1.tar+gzip", "size": 2000},
					{"mediaType": "application/vnd.oci.image.layer.v1.tar+zstd", "size": 3000}
				]
			}`,
			wantLayers: 3,
			wantBytes:  6000,
			wantGzip:   2,
			wantZstd:   1,
			wantErr:    false,
		},
		{
			name:       "empty layers",
			json:       `{"layers": []}`,
			wantLayers: 0,
			wantBytes:  0,
			wantGzip:   0,
			wantZstd:   0,
			wantErr:    false,
		},
		{
			name:    "invalid json",
			json:    `{invalid`,
			wantErr: true,
		},
	}

	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			info, err := ExtractLayerMediaTypes([]byte(tc.json))
			if tc.wantErr {
				if err == nil {
					t.Fatal("expected error, got nil")
				}
				return
			}
			if err != nil {
				t.Fatalf("unexpected error: %v", err)
			}
			if info.TotalLayers != tc.wantLayers || info.TotalBytes != tc.wantBytes {
				t.Errorf("got layers=%d bytes=%d; want layers=%d bytes=%d",
					info.TotalLayers, info.TotalBytes, tc.wantLayers, tc.wantBytes)
			}
			if info.Compressions["gzip"] != tc.wantGzip || info.Compressions["zstd"] != tc.wantZstd {
				t.Errorf("got gzip=%d zstd=%d; want gzip=%d zstd=%d",
					info.Compressions["gzip"], info.Compressions["zstd"], tc.wantGzip, tc.wantZstd)
			}
		})
	}
}

func TestFormatLayerMediaTypes(t *testing.T) {
	jsonBlob := `{"layers":[{"mediaType":"application/vnd.oci.image.layer.v1.tar+gzip","size":1048576}]}`
	got := FormatLayerMediaTypes([]byte(jsonBlob))
	if !strings.Contains(got, "1 (1.00 MB)") {
		t.Errorf("expected 1 (1.00 MB) in %q", got)
	}
	if !strings.Contains(got, "gzip: 1") {
		t.Errorf("expected gzip: 1 in %q", got)
	}
}
