package imagestore

import (
	"testing"
)

func TestInspectEmptyLayers(t *testing.T) {
	tests := []struct {
		name        string
		json        string
		wantTotal   int
		wantEmpty   int
		wantData    int
		wantErr     bool
	}{
		{
			name: "mixed layers",
			json: `{
				"history": [
					{"created_by": "ADD file:... in /", "empty_layer": false},
					{"created_by": "ENV PATH=/usr/local/bin:$PATH", "empty_layer": true},
					{"created_by": "RUN apt-get update", "empty_layer": false},
					{"created_by": "WORKDIR /app", "empty_layer": true},
					{"created_by": "CMD [\"sh\"]", "empty_layer": true}
				]
			}`,
			wantTotal: 5,
			wantEmpty: 3,
			wantData:  2,
			wantErr:   false,
		},
		{
			name: "all data layers",
			json: `{
				"history": [
					{"created_by": "ADD base.tar.gz /"},
					{"created_by": "RUN make install"}
				]
			}`,
			wantTotal: 2,
			wantEmpty: 0,
			wantData:  2,
			wantErr:   false,
		},
		{
			name: "empty history array",
			json: `{"history": []}`,
			wantTotal: 0,
			wantEmpty: 0,
			wantData:  0,
			wantErr:   false,
		},
		{
			name: "missing history field",
			json: `{"config": {}}`,
			wantTotal: 0,
			wantEmpty: 0,
			wantData:  0,
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
			summary, err := InspectEmptyLayers([]byte(tc.json))
			if tc.wantErr {
				if err == nil {
					t.Fatal("expected error, got nil")
				}
				return
			}
			if err != nil {
				t.Fatalf("unexpected error: %v", err)
			}
			if summary.TotalLayers != tc.wantTotal {
				t.Errorf("TotalLayers = %d, want %d", summary.TotalLayers, tc.wantTotal)
			}
			if summary.EmptyLayers != tc.wantEmpty {
				t.Errorf("EmptyLayers = %d, want %d", summary.EmptyLayers, tc.wantEmpty)
			}
			if summary.DataLayers != tc.wantData {
				t.Errorf("DataLayers = %d, want %d", summary.DataLayers, tc.wantData)
			}
		})
	}
}

func TestFormatEmptyLayerSummary(t *testing.T) {
	jsonBlob := `{
		"history": [
			{"created_by": "ADD file:... in /", "empty_layer": false},
			{"created_by": "ENV FOO=BAR", "empty_layer": true}
		]
	}`

	got := FormatEmptyLayerSummary([]byte(jsonBlob))
	want := "Layers: 2 total (1 data, 1 metadata-only)"
	if got != want {
		t.Errorf("FormatEmptyLayerSummary() = %q, want %q", got, want)
	}

	errOutput := FormatEmptyLayerSummary([]byte(`{invalid`))
	if errOutput == "" || errOutput == want {
		t.Errorf("expected error output, got %q", errOutput)
	}
}
