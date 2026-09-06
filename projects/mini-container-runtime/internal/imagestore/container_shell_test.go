package imagestore

import (
	"testing"
)

func TestExtractShell(t *testing.T) {
	configJSON := []byte(`{
		"config": {
			"Shell": ["/bin/bash", "-c"]
		}
	}`)

	sh := ExtractShell(configJSON)
	if len(sh) != 2 || sh[0] != "/bin/bash" {
		t.Fatalf("ExtractShell = %v, want [/bin/bash -c]", sh)
	}
}
