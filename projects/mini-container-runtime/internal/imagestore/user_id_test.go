package imagestore

import (
	"testing"
)

func TestExtractImageUser(t *testing.T) {
	configJSON := []byte(`{
		"config": {
			"User": "1000:1000"
		}
	}`)

	usr := ExtractImageUser(configJSON)
	if usr != "1000:1000" {
		t.Fatalf("ExtractImageUser = %s, want 1000:1000", usr)
	}
}
