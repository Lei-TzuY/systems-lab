package network

import (
	"fmt"
)

// GenerateDNSPortConfig formats nameserver entries with custom port.
func GenerateDNSPortConfig(ip string, port int) string {
	if ip == "" {
		return ""
	}
	if port <= 0 || port == 53 {
		return fmt.Sprintf("nameserver %s\n", ip)
	}
	return fmt.Sprintf("nameserver %s:%d\n", ip, port)
}
