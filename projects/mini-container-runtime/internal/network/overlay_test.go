package network

import (
	"strings"
	"testing"
)

func TestCreateOverlayInterface(t *testing.T) {
	cfg := OverlayConfig{
		VNI:           100,
		RemoteIP:      "192.168.1.50",
		InterfaceName: "vxlan100",
	}

	err := CreateOverlayInterface(cfg)
	if err != nil {
		msg := strings.ToLower(err.Error())
		if strings.Contains(msg, "operation not permitted") || strings.Contains(msg, "permission denied") {
			t.Skipf("requires network-admin capability: %v", err)
		}
		t.Fatalf("CreateOverlayInterface error: %v", err)
	}
}
