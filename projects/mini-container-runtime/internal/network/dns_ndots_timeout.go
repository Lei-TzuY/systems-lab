package network

import (
	"fmt"
)

// GenerateDNSNdotsTimeoutConfig formats combined options ndots:N timeout:T flags
// for /etc/resolv.conf. ndots specifies the threshold for absolute domain name
// lookups; timeout specifies the resolver wait duration in seconds.
func GenerateDNSNdotsTimeoutConfig(ndots, timeout int) string {
	if ndots < 0 {
		ndots = 1
	}
	if timeout <= 0 {
		timeout = 5
	}
	return fmt.Sprintf("options ndots:%d timeout:%d\n", ndots, timeout)
}
