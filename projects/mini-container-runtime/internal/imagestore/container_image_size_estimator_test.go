package imagestore

import (
	"strings"
	"testing"
)

func TestEstimateImageSizes(t *testing.T) {
	manifestJSON := `{
		"schemaVersion": 2,
		"config": {"size": 2048},
		"layers": [
			{"size": 10000000},
			{"size": 20000000}
		]
	}`

	est, err := EstimateImageSizes([]byte(manifestJSON), nil)
	if err != nil {
		t.Fatalf("EstimateImageSizes failed: %v", err)
	}

	if est.LayerCount != 2 {
		t.Errorf("LayerCount = %d, want 2", est.LayerCount)
	}
	if est.TotalDownload != 30002048 {
		t.Errorf("TotalDownload = %d, want 30002048", est.TotalDownload)
	}
	if est.EstimatedDisk <= est.TotalDownload {
		t.Errorf("EstimatedDisk (%d) should exceed TotalDownload (%d)", est.EstimatedDisk, est.TotalDownload)
	}
}

func TestFormatImageSizeEstimate(t *testing.T) {
	manifestJSON := `{"config":{"size":1000},"layers":[{"size":10485760}]}`
	got := FormatImageSizeEstimate([]byte(manifestJSON))
	if !strings.Contains(got, "10.00 MB") {
		t.Errorf("expected 10.00 MB in %q", got)
	}
	if !strings.Contains(got, "Estimated Unpacked Disk:") {
		t.Errorf("expected Estimated Unpacked Disk in %q", got)
	}
}
