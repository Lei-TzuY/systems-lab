package network

import (
	"testing"
)

func TestGenerateDNSRotateConfig(t *testing.T) {
	cfg := GenerateDNSRotateConfig()
	if cfg != "options rotate\n" {
		t.Fatalf("GenerateDNSRotateConfig = %s, want options rotate\\n", cfg)
	}
}
