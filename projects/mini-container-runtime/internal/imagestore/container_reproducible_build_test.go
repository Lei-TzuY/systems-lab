package imagestore

import (
	"strings"
	"testing"
)

func TestAuditReproducibleBuild(t *testing.T) {
	tests := []struct {
		name       string
		json       string
		wantScore  int
		wantZeroTs bool
		wantSorted bool
		wantErr    bool
	}{
		{
			name: "fully reproducible image (epoch zero, sorted env)",
			json: `{
				"created": "1970-01-01T00:00:00Z",
				"config": {
					"Env": ["FOO=1", "PATH=/bin", "ZOO=2"]
				}
			}`,
			wantScore:  100,
			wantZeroTs: true,
			wantSorted: true,
			wantErr:    false,
		},
		{
			name: "non-deterministic timestamp, unsorted env",
			json: `{
				"created": "2026-08-20T12:00:00Z",
				"config": {
					"Env": ["PATH=/bin", "FOO=1"]
				}
			}`,
			wantScore:  0,
			wantZeroTs: false,
			wantSorted: false,
			wantErr:    false,
		},
	}

	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			report, err := AuditReproducibleBuild([]byte(tc.json))
			if tc.wantErr {
				if err == nil {
					t.Fatal("expected error, got nil")
				}
				return
			}
			if err != nil {
				t.Fatalf("unexpected error: %v", err)
			}
			if report.ReproducibleScore != tc.wantScore {
				t.Errorf("ReproducibleScore = %d, want %d", report.ReproducibleScore, tc.wantScore)
			}
			if report.IsZeroTimestamp != tc.wantZeroTs {
				t.Errorf("IsZeroTimestamp = %t, want %t", report.IsZeroTimestamp, tc.wantZeroTs)
			}
			if report.SortedEnv != tc.wantSorted {
				t.Errorf("SortedEnv = %t, want %t", report.SortedEnv, tc.wantSorted)
			}
		})
	}
}

func TestFormatReproducibilityReport(t *testing.T) {
	jsonBlob := `{"created":"1970-01-01T00:00:00Z","config":{"Env":["A=1","B=2"]}}`
	got := FormatReproducibilityReport([]byte(jsonBlob))
	if !strings.Contains(got, "Score: 100/100") {
		t.Errorf("expected Score: 100/100 in %q", got)
	}
}
