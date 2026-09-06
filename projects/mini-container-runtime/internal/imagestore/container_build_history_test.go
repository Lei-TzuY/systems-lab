package imagestore

import (
	"strings"
	"testing"
)

func TestExtractBuildHistory(t *testing.T) {
	tests := []struct {
		name      string
		json      string
		wantCount int
		wantErr   bool
	}{
		{
			name: "three layers",
			json: `{
				"history": [
					{"created_by": "/bin/sh -c #(nop) ADD file:abc"},
					{"created_by": "/bin/sh -c apt-get update"},
					{"created_by": "/bin/sh -c #(nop) CMD [\"bash\"]", "empty_layer": true}
				]
			}`,
			wantCount: 3,
			wantErr:   false,
		},
		{
			name:      "no history",
			json:      `{}`,
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
			entries, err := ExtractBuildHistory([]byte(tc.json))
			if tc.wantErr {
				if err == nil {
					t.Fatal("expected error, got nil")
				}
				return
			}
			if err != nil {
				t.Fatalf("unexpected error: %v", err)
			}
			if len(entries) != tc.wantCount {
				t.Errorf("got %d entries, want %d", len(entries), tc.wantCount)
			}
		})
	}
}

func TestFormatBuildHistory(t *testing.T) {
	jsonBlob := `{"history":[
		{"created_by":"RUN apt-get update"},
		{"created_by":"CMD bash","empty_layer":true}
	]}`
	got := FormatBuildHistory([]byte(jsonBlob))
	if !strings.Contains(got, "2 layer(s)") {
		t.Errorf("expected '2 layer(s)' in output, got %q", got)
	}
	if !strings.Contains(got, "META") {
		t.Errorf("expected META marker for empty layer, got %q", got)
	}
	if !strings.Contains(got, "LAYER") {
		t.Errorf("expected LAYER marker, got %q", got)
	}
}
