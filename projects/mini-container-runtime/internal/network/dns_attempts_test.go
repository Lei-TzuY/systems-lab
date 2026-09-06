package network

import (
	"testing"
)

func TestGenerateDNSAttemptsConfig(t *testing.T) {
	cfg := GenerateDNSAttemptsConfig(3)
	if cfg != "options attempts:3\n" {
		t.Fatalf("GenerateDNSAttemptsConfig = %s, want options attempts:3\\n", cfg)
	}
}
