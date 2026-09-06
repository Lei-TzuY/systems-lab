package network

import (
	"fmt"
)

// GenerateDNSInet6AttemptsTimeoutConfig formats combined options inet6 attempts:N timeout:M flags
// for /etc/resolv.conf. inet6 prioritizes IPv6 AAAA lookups; attempts sets query retries;
// timeout sets query timeout ceiling in seconds.
func GenerateDNSInet6AttemptsTimeoutConfig(attempts, timeoutSeconds int) string {
	if attempts <= 0 {
		attempts = 2
	}
	if timeoutSeconds <= 0 {
		timeoutSeconds = 5
	}
	return fmt.Sprintf("options inet6 attempts:%d timeout:%d\n", attempts, timeoutSeconds)
}
