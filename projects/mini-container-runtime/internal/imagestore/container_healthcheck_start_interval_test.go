package imagestore

import (
	"testing"
	"time"
)

func TestExtractHealthcheckStartInterval(t *testing.T) {
	tests := []struct {
		name         string
		json         string
		wantDuration time.Duration
		wantOk       bool
		wantErr      bool
	}{
		{
			name: "explicitly configured start interval (5s)",
			json: `{
				"config": {
					"Healthcheck": {
						"StartInterval": 5000000000
					}
				}
			}`,
			wantDuration: 5 * time.Second,
			wantOk:       true,
			wantErr:      false,
		},
		{
			name:         "not configured",
			json:         `{"config": {}}`,
			wantDuration: 0,
			wantOk:       false,
			wantErr:      false,
		},
		{
			name:         "zero duration treated as not set",
			json:         `{"config": {"Healthcheck": {"StartInterval": 0}}}`,
			wantDuration: 0,
			wantOk:       false,
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
			d, ok, err := ExtractHealthcheckStartInterval([]byte(tc.json))
			if tc.wantErr {
				if err == nil {
					t.Fatal("expected error, got nil")
				}
				return
			}
			if err != nil {
				t.Fatalf("unexpected error: %v", err)
			}
			if ok != tc.wantOk || d != tc.wantDuration {
				t.Errorf("got (%v, %t), want (%v, %t)", d, ok, tc.wantDuration, tc.wantOk)
			}
		})
	}
}

func TestFormatHealthcheckStartInterval(t *testing.T) {
	jsonBlob := `{"config":{"Healthcheck":{"StartInterval":2000000000}}}`
	got := FormatHealthcheckStartInterval([]byte(jsonBlob))
	want := "Healthcheck StartInterval: 2s"
	if got != want {
		t.Errorf("got %q, want %q", got, want)
	}

	notSet := FormatHealthcheckStartInterval([]byte(`{"config":{}}`))
	if notSet != "Healthcheck StartInterval: (not set)" {
		t.Errorf("got %q", notSet)
	}
}
