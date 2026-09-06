package imagestore

import (
	"testing"
	"time"
)

func TestExtractHealthcheckTimings(t *testing.T) {
	tests := []struct {
		name            string
		json            string
		wantConfigured  bool
		wantInterval    time.Duration
		wantTimeout     time.Duration
		wantStartPeriod time.Duration
		wantRetries     int
		wantErr         bool
	}{
		{
			name: "full timings set",
			json: `{
				"config": {
					"Healthcheck": {
						"Interval": 30000000000,
						"Timeout": 5000000000,
						"StartPeriod": 10000000000,
						"Retries": 3
					}
				}
			}`,
			wantConfigured:  true,
			wantInterval:    30 * time.Second,
			wantTimeout:     5 * time.Second,
			wantStartPeriod: 10 * time.Second,
			wantRetries:     3,
			wantErr:         false,
		},
		{
			name:           "not configured",
			json:           `{"config": {}}`,
			wantConfigured: false,
			wantErr:        false,
		},
		{
			name:    "invalid json",
			json:    `{invalid`,
			wantErr: true,
		},
	}

	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			info, err := ExtractHealthcheckTimings([]byte(tc.json))
			if tc.wantErr {
				if err == nil {
					t.Fatal("expected error, got nil")
				}
				return
			}
			if err != nil {
				t.Fatalf("unexpected error: %v", err)
			}
			if info.Configured != tc.wantConfigured {
				t.Errorf("Configured = %t, want %t", info.Configured, tc.wantConfigured)
			}
			if tc.wantConfigured {
				if info.Interval != tc.wantInterval || info.Timeout != tc.wantTimeout ||
					info.StartPeriod != tc.wantStartPeriod || info.Retries != tc.wantRetries {
					t.Errorf("got %+v, want interval=%v, timeout=%v, startPeriod=%v, retries=%d",
						info, tc.wantInterval, tc.wantTimeout, tc.wantStartPeriod, tc.wantRetries)
				}
			}
		})
	}
}

func TestFormatHealthcheckTimings(t *testing.T) {
	jsonBlob := `{
		"config": {
			"Healthcheck": {
				"Interval": 15000000000,
				"Timeout": 3000000000,
				"StartPeriod": 0,
				"Retries": 5
			}
		}
	}`

	got := FormatHealthcheckTimings([]byte(jsonBlob))
	want := "Healthcheck Timings: interval=15s, timeout=3s, start_period=0s, retries=5"
	if got != want {
		t.Errorf("got %q, want %q", got, want)
	}

	notConfigured := FormatHealthcheckTimings([]byte(`{"config":{}}`))
	if notConfigured != "Healthcheck Timings: (not configured)" {
		t.Errorf("got %q", notConfigured)
	}
}
