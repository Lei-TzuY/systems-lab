package network

import (
	"fmt"
	"strings"
)

// GenerateDNSOptionsConfig formats custom DNS options directives.
func GenerateDNSOptionsConfig(opts []string) string {
	if len(opts) == 0 {
		return ""
	}
	return fmt.Sprintf("options %s\n", strings.Join(opts, " "))
}
