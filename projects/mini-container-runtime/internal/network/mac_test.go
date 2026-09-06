package network

import (
	"strings"
	"testing"
)

func TestGenerateMACAddress(t *testing.T) {
	mac := GenerateMACAddress("172.20.0.5")
	if !strings.HasPrefix(mac, "02:42:") {
		t.Fatalf("GenerateMACAddress = %s, want prefix 02:42:", mac)
	}
}
