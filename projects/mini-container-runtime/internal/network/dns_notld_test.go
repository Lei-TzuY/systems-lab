package network

import (
	"testing"
)

func TestGenerateDNSNoTLDConfig(t *testing.T) {
	cfg := GenerateDNSNoTLDConfig()
	if cfg != "options no-tld-query\n" {
		t.Fatalf("GenerateDNSNoTLDConfig = %s, want options no-tld-query\\n", cfg)
	}
}
