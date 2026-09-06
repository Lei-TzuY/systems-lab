package network

import (
	"fmt"
	"strings"
)

// GenerateDNSFallbackConfig formats fallback upstream nameserver entries.
func GenerateDNSFallbackConfig(servers []string) string {
	if len(servers) == 0 {
		servers = []string{"8.8.8.8", "1.1.1.1"}
	}
	var sb strings.Builder
	for _, ns := range servers {
		sb.WriteString(fmt.Sprintf("nameserver %s\n", ns))
	}
	return sb.String()
}
