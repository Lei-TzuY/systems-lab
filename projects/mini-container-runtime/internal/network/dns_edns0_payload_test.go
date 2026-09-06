package network

import (
	"testing"
)

func TestGenerateDNSEDNS0PayloadConfig(t *testing.T) {
	cfg := GenerateDNSEDNS0PayloadConfig(1232)
	if cfg != "options edns0-payload:1232\n" {
		t.Fatalf("GenerateDNSEDNS0PayloadConfig = %s, want options edns0-payload:1232\\n", cfg)
	}
}
