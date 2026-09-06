package imagestore

import (
	"testing"
)

func TestExtractStopSignal(t *testing.T) {
	configJSON := []byte(`{
		"config": {
			"StopSignal": "SIGQUIT"
		}
	}`)

	sig := ExtractStopSignal(configJSON)
	if sig != "SIGQUIT" {
		t.Fatalf("ExtractStopSignal = %s, want SIGQUIT", sig)
	}
}
