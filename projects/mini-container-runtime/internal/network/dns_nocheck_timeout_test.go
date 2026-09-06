package network

import (
	"testing"
)

func TestGenerateDNSNoCheckTimeoutConfig(t *testing.T) {
	cfg := GenerateDNSNoCheckTimeoutConfig(3)
	if cfg != "options no-check-names timeout:3\n" {
		t.Fatalf("GenerateDNSNoCheckTimeoutConfig = %s, want options no-check-names timeout:3\\n", cfg)
	}
}
