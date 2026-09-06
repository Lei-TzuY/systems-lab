package imagestore

import (
	"strings"
	"testing"
)

func TestExtractManifestSchema(t *testing.T) {
	tests := []struct {
		name        string
		json        string
		wantVersion int
		wantMedia   string
		wantFormat  string
		wantErr     bool
	}{
		{
			name: "oci image manifest v1",
			json: `{
				"schemaVersion": 2,
				"mediaType": "application/vnd.oci.image.manifest.v1+json"
			}`,
			wantVersion: 2,
			wantMedia:   "application/vnd.oci.image.manifest.v1+json",
			wantFormat:  "OCI v1",
			wantErr:     false,
		},
		{
			name: "docker manifest v2",
			json: `{
				"schemaVersion": 2,
				"mediaType": "application/vnd.docker.distribution.manifest.v2+json"
			}`,
			wantVersion: 2,
			wantMedia:   "application/vnd.docker.distribution.manifest.v2+json",
			wantFormat:  "Docker Manifest v2",
			wantErr:     false,
		},
		{
			name: "legacy v1",
			json: `{
				"schemaVersion": 1
			}`,
			wantVersion: 1,
			wantMedia:   "",
			wantFormat:  "Docker Manifest v1 (Legacy)",
			wantErr:     false,
		},
		{
			name:    "invalid json",
			json:    `{invalid`,
			wantErr: true,
		},
	}

	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			info, err := ExtractManifestSchema([]byte(tc.json))
			if tc.wantErr {
				if err == nil {
					t.Fatal("expected error, got nil")
				}
				return
			}
			if err != nil {
				t.Fatalf("unexpected error: %v", err)
			}
			if info.SchemaVersion != tc.wantVersion || info.MediaType != tc.wantMedia || info.Format != tc.wantFormat {
				t.Errorf("got (%d, %q, %q); want (%d, %q, %q)",
					info.SchemaVersion, info.MediaType, info.Format,
					tc.wantVersion, tc.wantMedia, tc.wantFormat)
			}
		})
	}
}

func TestFormatManifestSchema(t *testing.T) {
	jsonBlob := `{"schemaVersion":2,"mediaType":"application/vnd.oci.image.manifest.v1+json"}`
	got := FormatManifestSchema([]byte(jsonBlob))
	if !strings.Contains(got, "OCI v1") {
		t.Errorf("expected OCI v1 in %q", got)
	}
}
