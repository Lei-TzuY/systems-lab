package network

import (
	"testing"
)

func TestGenerateDNSInet6Config(t *testing.T) {
	cfg := GenerateDNSInet6Config()
	if cfg != "options inet6\n" {
		t.Fatalf("GenerateDNSInet6Config = %s, want options inet6\\n", cfg)
	}
}
