package network

import (
	"testing"
)

func TestGenerateDNSOptionsConfig(t *testing.T) {
	cfg := GenerateDNSOptionsConfig([]string{"timeout:2", "ndots:5"})
	if cfg != "options timeout:2 ndots:5\n" {
		t.Fatalf("GenerateDNSOptionsConfig = %s, want options timeout:2 ndots:5\\n", cfg)
	}
}
