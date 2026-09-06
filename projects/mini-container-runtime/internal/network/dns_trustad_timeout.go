package network

import (
	"fmt"
)

// GenerateDNSTrustADTimeoutConfig formats combined options trust-ad timeout:N flags
// for /etc/resolv.conf. trust-ad sets the AD (Authenticated Data) bit in DNS requests
// to enable validation trust; timeout sets the resolver response wait limit in seconds.
func GenerateDNSTrustADTimeoutConfig(timeout int) string {
	if timeout <= 0 {
		timeout = 5
	}
	return fmt.Sprintf("options trust-ad timeout:%d\n", timeout)
}
