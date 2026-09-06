package network

import (
	"fmt"
)

// GenerateDNSSingleReqAttemptsTimeoutConfig formats combined options single-request attempts:N timeout:M flags
// for /etc/resolv.conf. single-request disables parallel A and AAAA lookups; attempts sets query retries;
// timeout sets query timeout ceiling in seconds.
func GenerateDNSSingleReqAttemptsTimeoutConfig(attempts, timeoutSeconds int) string {
	if attempts <= 0 {
		attempts = 2
	}
	if timeoutSeconds <= 0 {
		timeoutSeconds = 5
	}
	return fmt.Sprintf("options single-request attempts:%d timeout:%d\n", attempts, timeoutSeconds)
}
