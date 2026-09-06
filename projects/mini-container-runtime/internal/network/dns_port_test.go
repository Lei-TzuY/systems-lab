package network

import (
	"testing"
)

func TestGenerateDNSPortConfig(t *testing.T) {
	cfg := GenerateDNSPortConfig("10.0.0.1", 5353)
	if cfg != "nameserver 10.0.0.1:5353\n" {
		t.Fatalf("GenerateDNSPortConfig = %s, want nameserver 10.0.0.1:5353\\n", cfg)
	}
}
