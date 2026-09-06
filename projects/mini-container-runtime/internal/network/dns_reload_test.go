package network

import (
	"testing"
)

func TestGenerateDNSReloadConfig(t *testing.T) {
	cfg := GenerateDNSReloadConfig()
	if cfg != "options reload\n" {
		t.Fatalf("GenerateDNSReloadConfig = %s, want options reload\\n", cfg)
	}
}
