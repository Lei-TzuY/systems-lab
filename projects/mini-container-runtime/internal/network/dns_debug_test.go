package network

import (
	"testing"
)

func TestGenerateDNSDebugConfig(t *testing.T) {
	cfg := GenerateDNSDebugConfig()
	if cfg != "options debug\n" {
		t.Fatalf("GenerateDNSDebugConfig = %s, want options debug\\n", cfg)
	}
}
