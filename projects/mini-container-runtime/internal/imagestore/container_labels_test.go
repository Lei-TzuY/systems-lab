package imagestore

import (
	"testing"
)

func TestExtractLabels(t *testing.T) {
	configJSON := []byte(`{
		"config": {
			"Labels": {
				"maintainer": "alice@example.com",
				"version": "1.2.3"
			}
		}
	}`)

	labels, err := ExtractLabels(configJSON)
	if err != nil || len(labels) != 2 || labels["version"] != "1.2.3" {
		t.Fatalf("ExtractLabels error: %v, labels: %v", err, labels)
	}
}
