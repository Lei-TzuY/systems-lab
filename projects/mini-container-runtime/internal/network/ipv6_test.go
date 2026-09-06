package network

import (
	"strings"
	"testing"
)

func TestGenerateIPv6Address(t *testing.T) {
	addr := GenerateIPv6Address(10)
	if !strings.HasPrefix(addr, "2001:db8:1::a") {
		t.Fatalf("GenerateIPv6Address = %s, want prefix 2001:db8:1::a", addr)
	}
}
