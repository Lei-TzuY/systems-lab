package network

import (
	"testing"
)

func TestGenerateDNSRotateTimeoutConfig(t *testing.T) {
	cfg := GenerateDNSRotateTimeoutConfig(3)
	if cfg != "options rotate timeout:3\n" {
		t.Fatalf("GenerateDNSRotateTimeoutConfig = %s, want options rotate timeout:3\\n", cfg)
	}
}
