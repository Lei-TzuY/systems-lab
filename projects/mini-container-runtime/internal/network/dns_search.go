package network

import (
	"fmt"
	"strings"
)

// GenerateDNSSearchConfig formats custom DNS search domain suffixes.
func GenerateDNSSearchConfig(domains []string) string {
	if len(domains) == 0 {
		return ""
	}
	return fmt.Sprintf("search %s\n", strings.Join(domains, " "))
}
