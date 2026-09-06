package imagestore

import (
	"reflect"
	"testing"
)

func TestExtractOSCompatibility(t *testing.T) {
	tests := []struct {
		name         string
		json         string
		wantOS       string
		wantVersion  string
		wantFeatures []string
		wantErr      bool
	}{
		{
			name: "windows container with version and features",
			json: `{
				"os": "windows",
				"os.version": "10.0.20348.1",
				"os.features": ["win32k", "hyperv"]
			}`,
			wantOS:       "windows",
			wantVersion:  "10.0.20348.1",
			wantFeatures: []string{"win32k", "hyperv"},
			wantErr:      false,
		},
		{
			name:         "linux default",
			json:         `{"config": {}}`,
			wantOS:       "linux",
			wantVersion:  "",
			wantFeatures: nil,
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
			info, err := ExtractOSCompatibility([]byte(tc.json))
			if tc.wantErr {
				if err == nil {
					t.Fatal("expected error, got nil")
				}
				return
			}
			if err != nil {
				t.Fatalf("unexpected error: %v", err)
			}
			if info.OS != tc.wantOS || info.OSVersion != tc.wantVersion {
				t.Errorf("got OS=%s, Version=%s; want OS=%s, Version=%s",
					info.OS, info.OSVersion, tc.wantOS, tc.wantVersion)
			}
			if !reflect.DeepEqual(info.OSFeatures, tc.wantFeatures) {
				t.Errorf("got features %v, want %v", info.OSFeatures, tc.wantFeatures)
			}
		})
	}
}

func TestFormatOSCompatibility(t *testing.T) {
	jsonBlob := `{"os":"windows","os.version":"10.0.19041","os.features":["hyperv"]}`
	got := FormatOSCompatibility([]byte(jsonBlob))
	want := "OS Target: windows, Version: 10.0.19041, features=[hyperv]"
	if got != want {
		t.Errorf("got %q, want %q", got, want)
	}
}
