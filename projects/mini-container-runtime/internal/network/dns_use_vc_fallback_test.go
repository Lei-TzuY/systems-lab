package network

import (
	"testing"
)

func TestGenerateDNSUseVCFallbackConfig(t *testing.T) {
	cfg := GenerateDNSUseVCFallbackConfig()
	if cfg != "options use-vc\n" {
		t.Fatalf("GenerateDNSUseVCFallbackConfig = %s, want options use-vc\\n", cfg)
	}
}
