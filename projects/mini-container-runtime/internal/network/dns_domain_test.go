package network

import (
	"testing"
)

func TestGenerateDNSDomainConfig(t *testing.T) {
	cfg := GenerateDNSDomainConfig("localdomain")
	if cfg != "domain localdomain\n" {
		t.Fatalf("GenerateDNSDomainConfig = %s, want domain localdomain\\n", cfg)
	}
}
