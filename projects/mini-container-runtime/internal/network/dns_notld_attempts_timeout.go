package network

import (
	"fmt"
)

// GenerateDNSNoTLDAttemptsTimeoutConfig formats combined options no-tld-query attempts:N timeout:M flags
// for /etc/resolv.conf. no-tld-query prevents querying top-level domains; attempts sets retry counts;
// timeout sets query timeout ceiling in seconds.
func GenerateDNSNoTLDAttemptsTimeoutConfig(attempts, timeoutSeconds int) string {
	if attempts <= 0 {
		attempts = 2
	}
	if timeoutSeconds <= 0 {
		timeoutSeconds = 5
	}
	return fmt.Sprintf("options no-tld-query attempts:%d timeout:%d\n", attempts, timeoutSeconds)
}
