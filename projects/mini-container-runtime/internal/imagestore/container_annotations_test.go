package imagestore

import (
	"strings"
	"testing"
)

func TestExtractManifestAnnotations(t *testing.T) {
	tests := []struct {
		name        string
		json        string
		wantTitle   string
		wantVersion string
		wantVendor  string
		wantErr     bool
	}{
		{
			name: "standard oci annotations",
			json: `{
				"annotations": {
					"org.opencontainers.image.title": "my-microservice",
					"org.opencontainers.image.version": "v1.2.3",
					"org.opencontainers.image.vendor": "Acme Corp"
				}
			}`,
			wantTitle:   "my-microservice",
			wantVersion: "v1.2.3",
			wantVendor:  "Acme Corp",
			wantErr:     false,
		},
		{
			name:        "empty annotations",
			json:        `{}`,
			wantTitle:   "",
			wantVersion: "",
			wantVendor:  "",
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
			ann, err := ExtractManifestAnnotations([]byte(tc.json))
			if tc.wantErr {
				if err == nil {
					t.Fatal("expected error, got nil")
				}
				return
			}
			if err != nil {
				t.Fatalf("unexpected error: %v", err)
			}
			if ann.Title != tc.wantTitle || ann.Version != tc.wantVersion || ann.Vendor != tc.wantVendor {
				t.Errorf("got (%q, %q, %q); want (%q, %q, %q)",
					ann.Title, ann.Version, ann.Vendor,
					tc.wantTitle, tc.wantVersion, tc.wantVendor)
			}
		})
	}
}

func TestFormatManifestAnnotations(t *testing.T) {
	jsonBlob := `{"annotations":{"org.opencontainers.image.title":"app","org.opencontainers.image.version":"1.0"}}`
	got := FormatManifestAnnotations([]byte(jsonBlob))
	if !strings.Contains(got, "Title: app") {
		t.Errorf("expected 'Title: app' in %q", got)
	}
	if !strings.Contains(got, "Version: 1.0") {
		t.Errorf("expected 'Version: 1.0' in %q", got)
	}
}
