package network

import (
	"fmt"
)

// GenerateDNSTrustADAttemptsTimeoutConfig formats combined options trust-ad attempts:N timeout:M flags
// for /etc/resolv.conf. trust-ad trusts DNSSEC AD flags; attempts sets the query retries;
// timeout sets query timeout ceiling in seconds.
func GenerateDNSTrustADAttemptsTimeoutConfig(attempts, timeoutSeconds int) string {
	if attempts <= 0 {
		attempts = 2
	}
	if timeoutSeconds <= 0 {
		timeoutSeconds = 5
	}
	return fmt.Sprintf("options trust-ad attempts:%d timeout:%d\n", attempts, timeoutSeconds)
}
