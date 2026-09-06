package imagestore

import (
	"testing"
)

func TestExtractContainerConfig(t *testing.T) {
	configJSON := []byte(`{
		"container_config": {
			"Hostname": "build-container-123"
		}
	}`)

	cc, err := ExtractContainerConfig(configJSON)
	if err != nil || cc == nil || cc["Hostname"] != "build-container-123" {
		t.Fatalf("ExtractContainerConfig error: %v (cc=%v)", err, cc)
	}
}
