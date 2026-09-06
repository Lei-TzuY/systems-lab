package imagestore

import (
	"testing"
)

func TestExtractHealthcheck(t *testing.T) {
	configJSON := []byte(`{
		"config": {
			"Healthcheck": {
				"Test": ["CMD-SHELL", "curl -f http://localhost/ || exit 1"],
				"Interval": 30000000000,
				"Timeout": 3000000000,
				"Retries": 3
			}
		}
	}`)

	hc, err := ExtractHealthcheck(configJSON)
	if err != nil || hc == nil || len(hc.Test) == 0 {
		t.Fatalf("ExtractHealthcheck error: %v (hc=%v)", err, hc)
	}
}
