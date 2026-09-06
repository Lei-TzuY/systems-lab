package network

import (
	"fmt"
)

// GenerateDNSDebugAttemptsTimeoutConfig formats combined options debug attempts:N timeout:M flags
// for /etc/resolv.conf. debug enables resolver debugging mode; attempts sets query retries;
// timeout sets query timeout ceiling in seconds.
func GenerateDNSDebugAttemptsTimeoutConfig(attempts, timeoutSeconds int) string {
	if attempts <= 0 {
		attempts = 2
	}
	if timeoutSeconds <= 0 {
		timeoutSeconds = 5
	}
	return fmt.Sprintf("options debug attempts:%d timeout:%d\n", attempts, timeoutSeconds)
}
