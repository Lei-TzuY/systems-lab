package network

import (
	"testing"
)

func TestGenerateDNSAttemptsTimeoutConfig(t *testing.T) {
	cfg := GenerateDNSAttemptsTimeoutConfig(3, 2)
	if cfg != "options attempts:3 timeout:2\n" {
		t.Fatalf("GenerateDNSAttemptsTimeoutConfig = %s, want options attempts:3 timeout:2\\n", cfg)
	}
}
