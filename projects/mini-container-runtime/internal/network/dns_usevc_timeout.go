package network

import (
	"fmt"
)

// GenerateDNSUseVCTimeoutConfig formats combined options use-vc timeout:N flags
// for /etc/resolv.conf. use-vc forces the resolver to use TCP (virtual circuit)
// for all DNS queries; timeout sets the response wait limit in seconds.
func GenerateDNSUseVCTimeoutConfig(timeout int) string {
	if timeout <= 0 {
		timeout = 5
	}
	return fmt.Sprintf("options use-vc timeout:%d\n", timeout)
}
