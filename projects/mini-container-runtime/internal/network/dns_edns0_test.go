package network

import (
	"testing"
)

func TestGenerateDNSEDNS0Config(t *testing.T) {
	cfg := GenerateDNSEDNS0Config()
	if cfg != "options edns0\n" {
		t.Fatalf("GenerateDNSEDNS0Config = %s, want options edns0\\n", cfg)
	}
}
