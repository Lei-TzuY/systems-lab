package network

import (
	"testing"
)

func TestGenerateDNSNoReloadTimeoutConfig(t *testing.T) {
	cfg := GenerateDNSNoReloadTimeoutConfig(4)
	if cfg != "options no-reload timeout:4\n" {
		t.Fatalf("GenerateDNSNoReloadTimeoutConfig = %s, want options no-reload timeout:4\\n", cfg)
	}
}
