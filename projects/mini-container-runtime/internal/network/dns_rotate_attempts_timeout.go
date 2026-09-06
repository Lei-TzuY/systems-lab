package network

import (
	"fmt"
)

// GenerateDNSRotateAttemptsTimeoutConfig formats combined options rotate attempts:N timeout:M flags
// for /etc/resolv.conf. rotate causes round-robin selection among nameservers;
// attempts sets query retries; timeout sets query timeout ceiling in seconds.
func GenerateDNSRotateAttemptsTimeoutConfig(attempts, timeoutSeconds int) string {
	if attempts <= 0 {
		attempts = 2
	}
	if timeoutSeconds <= 0 {
		timeoutSeconds = 5
	}
	return fmt.Sprintf("options rotate attempts:%d timeout:%d\n", attempts, timeoutSeconds)
}
