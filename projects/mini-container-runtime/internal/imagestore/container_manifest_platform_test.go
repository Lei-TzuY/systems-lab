package imagestore

import (
	"strings"
	"testing"
)

func TestMatchPlatformManifest(t *testing.T) {
	manifestListJSON := `{
		"schemaVersion": 2,
		"mediaType": "application/vnd.oci.image.index.v1+json",
		"manifests": [
			{
				"mediaType": "application/vnd.oci.image.manifest.v1+json",
				"digest": "sha256:linux-amd64-digest",
				"size": 1200,
				"platform": {"os": "linux", "architecture": "amd64"}
			},
			{
				"mediaType": "application/vnd.oci.image.manifest.v1+json",
				"digest": "sha256:linux-arm64-v8-digest",
				"size": 1250,
				"platform": {"os": "linux", "architecture": "arm64", "variant": "v8"}
			},
			{
				"mediaType": "application/vnd.oci.image.manifest.v1+json",
				"digest": "sha256:windows-amd64-digest",
				"size": 2500,
				"platform": {"os": "windows", "architecture": "amd64"}
			}
		]
	}`

	tests := []struct {
		name        string
		targetOS    string
		targetArch  string
		targetVar   string
		wantDigest  string
		wantErr     bool
	}{
		{
			name:       "match linux amd64",
			targetOS:   "linux",
			targetArch: "amd64",
			targetVar:  "",
			wantDigest: "sha256:linux-amd64-digest",
			wantErr:    false,
		},
		{
			name:       "match linux arm64 v8",
			targetOS:   "linux",
			targetArch: "arm64",
			targetVar:  "v8",
			wantDigest: "sha256:linux-arm64-v8-digest",
			wantErr:    false,
		},
		{
			name:       "unsupported platform",
			targetOS:   "darwin",
			targetArch: "arm64",
			targetVar:  "",
			wantErr:    true,
		},
	}

	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			m, err := MatchPlatformManifest([]byte(manifestListJSON), tc.targetOS, tc.targetArch, tc.targetVar)
			if tc.wantErr {
				if err == nil {
					t.Fatal("expected error, got nil")
				}
				return
			}
			if err != nil {
				t.Fatalf("unexpected error: %v", err)
			}
			if m.Digest != tc.wantDigest {
				t.Errorf("got digest %q, want %q", m.Digest, tc.wantDigest)
			}
		})
	}
}

func TestFormatPlatformManifests(t *testing.T) {
	manifestListJSON := `{"manifests":[{"platform":{"os":"linux","architecture":"arm64","variant":"v8"},"digest":"sha256:abcdef1234567890"}]}`
	got := FormatPlatformManifests([]byte(manifestListJSON))
	if !strings.Contains(got, "linux/arm64/v8") {
		t.Errorf("expected linux/arm64/v8 in %q", got)
	}
}
