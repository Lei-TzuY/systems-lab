package network

import (
	"fmt"
)

// GenerateDNSDomainConfig formats custom DNS domain directives.
func GenerateDNSDomainConfig(domain string) string {
	if domain == "" {
		return ""
	}
	return fmt.Sprintf("domain %s\n", domain)
}
