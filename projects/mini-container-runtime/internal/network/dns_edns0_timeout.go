package network

import (
	"fmt"
)

// GenerateDNSEDNS0TimeoutConfig formats combined options edns0 timeout:N flags.
func GenerateDNSEDNS0TimeoutConfig(timeout int) string {
	if timeout <= 0 {
		timeout = 5
	}
	return fmt.Sprintf("options edns0 timeout:%d\n", timeout)
}
