package network

import (
	"fmt"
)

// GenerateDNSNoReloadAttemptsTimeoutConfig formats combined options no-reload attempts:N timeout:M flags
// for /etc/resolv.conf. no-reload disables stat-polling of resolv.conf; attempts sets query retries;
// timeout sets query timeout ceiling in seconds.
func GenerateDNSNoReloadAttemptsTimeoutConfig(attempts, timeoutSeconds int) string {
	if attempts <= 0 {
		attempts = 2
	}
	if timeoutSeconds <= 0 {
		timeoutSeconds = 5
	}
	return fmt.Sprintf("options no-reload attempts:%d timeout:%d\n", attempts, timeoutSeconds)
}
