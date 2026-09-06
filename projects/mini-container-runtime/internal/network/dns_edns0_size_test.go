package network

import (
	"testing"
)

func TestGenerateDNSEDNS0SizeConfig(t *testing.T) {
	cfg := GenerateDNSEDNS0SizeConfig(1232)
	if cfg != "options edns0-size:1232\n" {
		t.Fatalf("GenerateDNSEDNS0SizeConfig = %s, want options edns0-size:1232\\n", cfg)
	}
}
