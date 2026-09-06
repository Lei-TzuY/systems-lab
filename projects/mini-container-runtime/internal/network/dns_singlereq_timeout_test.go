package network

import (
	"testing"
)

func TestGenerateDNSSingleReqTimeoutConfig(t *testing.T) {
	cfg := GenerateDNSSingleReqTimeoutConfig(3)
	if cfg != "options single-request-reopen timeout:3\n" {
		t.Fatalf("GenerateDNSSingleReqTimeoutConfig = %s, want options single-request-reopen timeout:3\\n", cfg)
	}
}
