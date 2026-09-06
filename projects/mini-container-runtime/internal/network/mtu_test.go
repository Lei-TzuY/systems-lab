package network

import (
	"strings"
	"testing"
)

func TestSetInterfaceMTU(t *testing.T) {
	if err := SetInterfaceMTU("lo", 1500); err != nil {
		msg := strings.ToLower(err.Error())
		if strings.Contains(msg, "operation not permitted") || strings.Contains(msg, "permission denied") {
			t.Skipf("requires network-admin capability: %v", err)
		}
		t.Fatalf("SetInterfaceMTU error: %v", err)
	}
}
