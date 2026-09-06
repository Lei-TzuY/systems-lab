package network

import (
	"fmt"
)

// GenerateDNSNDotsConfig formats options ndots:N flags.
func GenerateDNSNDotsConfig(ndots int) string {
	if ndots <= 0 {
		ndots = 1
	}
	return fmt.Sprintf("options ndots:%d\n", ndots)
}
