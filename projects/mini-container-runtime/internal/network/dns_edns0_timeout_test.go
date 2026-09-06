package network

import (
	"testing"
)

func TestGenerateDNSEDNS0TimeoutConfig(t *testing.T) {
	cfg := GenerateDNSEDNS0TimeoutConfig(2)
	if cfg != "options edns0 timeout:2\n" {
		t.Fatalf("GenerateDNSEDNS0TimeoutConfig = %s, want options edns0 timeout:2\\n", cfg)
	}
}
