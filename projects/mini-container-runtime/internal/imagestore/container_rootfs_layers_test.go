package imagestore

import (
	"strings"
	"testing"
)

func TestExtractRootFSLayers(t *testing.T) {
	tests := []struct {
		name      string
		json      string
		wantType  string
		wantCount int
		wantErr   bool
	}{
		{
			name: "two layers",
			json: `{
				"rootfs": {
					"type": "layers",
					"diff_ids": [
						"sha256:abc123",
						"sha256:def456"
					]
				}
			}`,
			wantType:  "layers",
			wantCount: 2,
			wantErr:   false,
		},
		{
			name:      "no rootfs",
			json:      `{}`,
			wantType:  "",
			wantCount: 0,
			wantErr:   false,
		},
		{
			name:    "invalid json",
			json:    `{invalid`,
			wantErr: true,
		},
	}

	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			info, err := ExtractRootFSLayers([]byte(tc.json))
			if tc.wantErr {
				if err == nil {
					t.Fatal("expected error, got nil")
				}
				return
			}
			if err != nil {
				t.Fatalf("unexpected error: %v", err)
			}
			if info.Type != tc.wantType {
				t.Errorf("Type = %q, want %q", info.Type, tc.wantType)
			}
			if info.LayerCount != tc.wantCount {
				t.Errorf("LayerCount = %d, want %d", info.LayerCount, tc.wantCount)
			}
		})
	}
}

func TestFormatRootFSLayers(t *testing.T) {
	jsonBlob := `{"rootfs":{"type":"layers","diff_ids":["sha256:abcdef1234567890abcdef1234567890"]}}`
	got := FormatRootFSLayers([]byte(jsonBlob))
	if !strings.Contains(got, "Layers: 1") {
		t.Errorf("expected 'Layers: 1' in output, got %q", got)
	}
	if !strings.Contains(got, "[0]") {
		t.Errorf("expected layer index [0], got %q", got)
	}
}
