package network

import (
	"fmt"
)

// GenerateDNSNoCheckAttemptsTimeoutConfig formats combined options no-check-names attempts:N timeout:M flags
// for /etc/resolv.conf. no-check-names bypasses hostname character validation; attempts sets query retries;
// timeout sets query timeout ceiling in seconds.
func GenerateDNSNoCheckAttemptsTimeoutConfig(attempts, timeoutSeconds int) string {
	if attempts <= 0 {
		attempts = 2
	}
	if timeoutSeconds <= 0 {
		timeoutSeconds = 5
	}
	return fmt.Sprintf("options no-check-names attempts:%d timeout:%d\n", attempts, timeoutSeconds)
}
