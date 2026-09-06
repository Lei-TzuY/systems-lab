package network

import (
	"fmt"
)

// GenerateDNSNoTLDTimeoutConfig formats combined options no-tld-query timeout:N flags
// for /etc/resolv.conf. no-tld-query prevents querying unqualified single-label domains
// as top-level domains; timeout sets the resolver response wait limit in seconds.
func GenerateDNSNoTLDTimeoutConfig(timeout int) string {
	if timeout <= 0 {
		timeout = 5
	}
	return fmt.Sprintf("options no-tld-query timeout:%d\n", timeout)
}
