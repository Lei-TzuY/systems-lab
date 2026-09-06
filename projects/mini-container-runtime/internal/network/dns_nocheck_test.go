package network

import (
	"testing"
)

func TestGenerateDNSNoCheckConfig(t *testing.T) {
	cfg := GenerateDNSNoCheckConfig()
	if cfg != "options no-check-names\n" {
		t.Fatalf("GenerateDNSNoCheckConfig = %s, want options no-check-names\\n", cfg)
	}
}
