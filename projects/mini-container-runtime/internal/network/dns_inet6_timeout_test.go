package network

import (
	"testing"
)

func TestGenerateDNSInet6TimeoutConfig(t *testing.T) {
	cfg := GenerateDNSInet6TimeoutConfig(4)
	if cfg != "options inet6 timeout:4\n" {
		t.Fatalf("GenerateDNSInet6TimeoutConfig = %s, want options inet6 timeout:4\\n", cfg)
	}
}
