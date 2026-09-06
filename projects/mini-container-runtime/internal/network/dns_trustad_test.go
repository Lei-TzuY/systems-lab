package network

import (
	"testing"
)

func TestGenerateDNSTrustADConfig(t *testing.T) {
	cfg := GenerateDNSTrustADConfig()
	if cfg != "options trust-ad\n" {
		t.Fatalf("GenerateDNSTrustADConfig = %s, want options trust-ad\\n", cfg)
	}
}
