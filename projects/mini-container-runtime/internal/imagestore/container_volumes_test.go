package imagestore

import (
	"testing"
)

func TestExtractVolumes(t *testing.T) {
	configJSON := []byte(`{
		"config": {
			"Volumes": {
				"/data": {},
				"/var/log": {}
			}
		}
	}`)

	vols := ExtractVolumes(configJSON)
	if len(vols) != 2 || vols[0] != "/data" || vols[1] != "/var/log" {
		t.Fatalf("ExtractVolumes = %v, want [/data /var/log]", vols)
	}
}
