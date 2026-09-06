package network

import (
	"testing"
)

func TestGenerateDNSTimeoutConfig(t *testing.T) {
	cfg := GenerateDNSTimeoutConfig(5)
	if cfg != "options timeout:5\n" {
		t.Fatalf("GenerateDNSTimeoutConfig = %s, want options timeout:5\\n", cfg)
	}
}
