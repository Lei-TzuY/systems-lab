package imagestore

import (
	"testing"
)

func TestExtractOSVersion(t *testing.T) {
	configJSON := []byte(`{
		"os": "windows",
		"os.version": "10.0.19041.1110"
	}`)

	ver := ExtractOSVersion(configJSON)
	if ver != "10.0.19041.1110" {
		t.Fatalf("ExtractOSVersion = %s, want 10.0.19041.1110", ver)
	}
}
