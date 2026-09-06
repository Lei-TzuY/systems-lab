package network

import (
	"fmt"
)

// GenerateDNSEDNS0AttemptsTimeoutConfig formats combined options edns0 attempts:N timeout:M flags
// for /etc/resolv.conf. edns0 enables Extended DNS buffer sizes; attempts sets query retries;
// timeout sets query timeout ceiling in seconds.
func GenerateDNSEDNS0AttemptsTimeoutConfig(attempts, timeoutSeconds int) string {
	if attempts <= 0 {
		attempts = 2
	}
	if timeoutSeconds <= 0 {
		timeoutSeconds = 5
	}
	return fmt.Sprintf("options edns0 attempts:%d timeout:%d\n", attempts, timeoutSeconds)
}
