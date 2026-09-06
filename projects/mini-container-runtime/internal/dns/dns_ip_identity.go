package dns

import (
	"fmt"
	"net/netip"
)

func canonicalIPAddress(ipAddr string) (string, error) {
	if ipAddr == "" {
		return "", fmt.Errorf("IP address cannot be empty")
	}
	addr, err := netip.ParseAddr(ipAddr)
	if err != nil || addr.Zone() != "" {
		return "", fmt.Errorf("invalid IP address %q", ipAddr)
	}
	return addr.String(), nil
}
