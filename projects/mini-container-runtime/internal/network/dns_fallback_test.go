package network

import (
	"strings"
	"testing"
)

func TestGenerateDNSFallbackConfig(t *testing.T) {
	cfg := GenerateDNSFallbackConfig([]string{"8.8.8.8"})
	if !strings.Contains(cfg, "nameserver 8.8.8.8") {
		t.Fatalf("GenerateDNSFallbackConfig = %s, want nameserver 8.8.8.8", cfg)
	}
}
