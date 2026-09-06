package logs

import (
	"testing"
)

func TestCleanANSICodes(t *testing.T) {
	input := "\x1b[31mError:\x1b[0m Failed to connect"
	cleaned := CleanANSICodes(input)
	if cleaned != "Error: Failed to connect" {
		t.Fatalf("CleanANSICodes = %q, want %q", cleaned, "Error: Failed to connect")
	}
}
