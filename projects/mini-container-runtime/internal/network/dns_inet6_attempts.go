package network

import (
	"fmt"
)

// GenerateDNSInet6AttemptsConfig formats combined options inet6 attempts:N flags
// for /etc/resolv.conf. inet6 causes the resolver to prefer IPv6 (AAAA) lookups;
// attempts sets the number of query retries before failure.
func GenerateDNSInet6AttemptsConfig(attempts int) string {
	if attempts <= 0 {
		attempts = 2
	}
	return fmt.Sprintf("options inet6 attempts:%d\n", attempts)
}
