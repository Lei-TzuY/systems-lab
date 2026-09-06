package network

import (
	"testing"
)

func TestGenerateDNSNDotsConfig(t *testing.T) {
	cfg := GenerateDNSNDotsConfig(5)
	if cfg != "options ndots:5\n" {
		t.Fatalf("GenerateDNSNDotsConfig = %s, want options ndots:5\\n", cfg)
	}
}
