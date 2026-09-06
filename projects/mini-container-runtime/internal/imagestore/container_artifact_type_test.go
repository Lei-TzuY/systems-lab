package imagestore

import (
	"strings"
	"testing"
)

func TestExtractArtifactInfo(t *testing.T) {
	tests := []struct {
		name         string
		json         string
		wantType     string
		wantSubject  bool
		wantArtifact bool
		wantErr      bool
	}{
		{
			name: "cosign signature artifact",
			json: `{
				"schemaVersion": 2,
				"mediaType": "application/vnd.oci.image.manifest.v1+json",
				"artifactType": "application/vnd.dev.cosign.simplesigning.v1+json",
				"subject": {
					"mediaType": "application/vnd.oci.image.manifest.v1+json",
					"digest": "sha256:1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef",
					"size": 528
				}
			}`,
			wantType:     "application/vnd.dev.cosign.simplesigning.v1+json",
			wantSubject:  true,
			wantArtifact: true,
			wantErr:      false,
		},
		{
			name: "standard container image manifest",
			json: `{
				"schemaVersion": 2,
				"mediaType": "application/vnd.oci.image.manifest.v1+json",
				"config": {"mediaType": "application/vnd.oci.image.config.v1+json"}
			}`,
			wantType:     "",
			wantSubject:  false,
			wantArtifact: false,
			wantErr:      false,
		},
		{
			name:    "invalid json",
			json:    `{invalid`,
			wantErr: true,
		},
	}

	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			info, err := ExtractArtifactInfo([]byte(tc.json))
			if tc.wantErr {
				if err == nil {
					t.Fatal("expected error, got nil")
				}
				return
			}
			if err != nil {
				t.Fatalf("unexpected error: %v", err)
			}
			if info.ArtifactType != tc.wantType || info.HasSubject != tc.wantSubject || info.IsArtifact != tc.wantArtifact {
				t.Errorf("got (%q, %t, %t); want (%q, %t, %t)",
					info.ArtifactType, info.HasSubject, info.IsArtifact,
					tc.wantType, tc.wantSubject, tc.wantArtifact)
			}
		})
	}
}

func TestFormatArtifactInfo(t *testing.T) {
	t.Run("cosign artifact", func(t *testing.T) {
		jsonBlob := `{"artifactType":"application/spdx+json","subject":{"digest":"sha256:abc123def456"}}`
		got := FormatArtifactInfo([]byte(jsonBlob))
		if !strings.Contains(got, "application/spdx+json") {
			t.Errorf("expected spdx in %q", got)
		}
	})

	t.Run("standard image", func(t *testing.T) {
		got := FormatArtifactInfo([]byte(`{}`))
		if got != "Artifact: standard container image" {
			t.Errorf("got %q", got)
		}
	})
}
