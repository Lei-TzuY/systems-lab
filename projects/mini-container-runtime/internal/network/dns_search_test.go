package network

import (
	"strings"
	"testing"
)

func TestGenerateDNSSearchConfig(t *testing.T) {
	cfg := GenerateDNSSearchConfig([]string{"default.svc.cluster.local", "svc.cluster.local"})
	if !strings.HasPrefix(cfg, "search default.svc.cluster.local") {
		t.Fatalf("GenerateDNSSearchConfig = %s, want search prefix", cfg)
	}
}
