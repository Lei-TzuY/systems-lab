package imagestore

import (
	"strings"
	"testing"
)

func TestAuditLabelPolicy_FullCompliance(t *testing.T) {
	configJSON := `{
		"config": {
			"Labels": {
				"org.opencontainers.image.title": "my-app",
				"org.opencontainers.image.description": "A cool app",
				"org.opencontainers.image.version": "1.2.3",
				"org.opencontainers.image.vendor": "ACME Corp",
				"org.opencontainers.image.url": "https://example.com",
				"org.opencontainers.image.source": "https://github.com/acme/app",
				"org.opencontainers.image.licenses": "MIT"
			}
		}
	}`

	report, err := AuditLabelPolicy([]byte(configJSON), nil)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if report.Score != 100 {
		t.Errorf("Score = %d, want 100", report.Score)
	}
	if report.Missing != 0 {
		t.Errorf("Missing = %d, want 0", report.Missing)
	}
}

func TestAuditLabelPolicy_PartialCompliance(t *testing.T) {
	configJSON := `{
		"config": {
			"Labels": {
				"org.opencontainers.image.title": "my-app",
				"org.opencontainers.image.version": "1.0.0"
			}
		}
	}`

	report, err := AuditLabelPolicy([]byte(configJSON), nil)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if report.Score >= 100 {
		t.Errorf("Score = %d, expected < 100 for partial compliance", report.Score)
	}
	if report.Missing != 5 {
		t.Errorf("Missing = %d, want 5", report.Missing)
	}
}

func TestAuditLabelPolicy_NoLabels(t *testing.T) {
	configJSON := `{"config":{}}`
	report, err := AuditLabelPolicy([]byte(configJSON), nil)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if report.Score != 0 {
		t.Errorf("Score = %d, want 0 for no labels", report.Score)
	}
}

func TestFormatLabelPolicyReport(t *testing.T) {
	configJSON := `{"config":{"Labels":{"org.opencontainers.image.title":"test"}}}`
	got := FormatLabelPolicyReport([]byte(configJSON))
	if !strings.Contains(got, "Label Policy Score:") {
		t.Errorf("expected policy score header in %q", got)
	}
}
