package network

import (
	"testing"
)

func TestGenerateDNSLoopbackConfig(t *testing.T) {
	cfg := GenerateDNSLoopbackConfig()
	if cfg != "nameserver 127.0.0.53\n" {
		t.Fatalf("GenerateDNSLoopbackConfig = %s, want nameserver 127.0.0.53\\n", cfg)
	}
}
