package network

import (
	"testing"
)

func TestGenerateDNSNoReloadConfig(t *testing.T) {
	cfg := GenerateDNSNoReloadConfig()
	if cfg != "options no-reload\n" {
		t.Fatalf("GenerateDNSNoReloadConfig = %s, want options no-reload\\n", cfg)
	}
}
