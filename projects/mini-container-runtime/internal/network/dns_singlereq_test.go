package network

import (
	"testing"
)

func TestGenerateDNSSingleReqConfig(t *testing.T) {
	cfg := GenerateDNSSingleReqConfig()
	if cfg != "options single-request-reopen\n" {
		t.Fatalf("GenerateDNSSingleReqConfig = %s, want options single-request-reopen\\n", cfg)
	}
}
