package network

import (
	"testing"
)

func TestGenerateDNSUseVCConfig(t *testing.T) {
	cfg := GenerateDNSUseVCConfig()
	if cfg != "options use-vc\n" {
		t.Fatalf("GenerateDNSUseVCConfig = %s, want options use-vc\\n", cfg)
	}
}
