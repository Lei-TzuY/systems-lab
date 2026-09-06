package network

import (
	"fmt"
)

// GenerateDNSUseVCAttemptsTimeoutConfig formats combined options use-vc attempts:N timeout:M flags
// for /etc/resolv.conf. use-vc forces TCP virtual circuits; attempts sets the query retry count;
// timeout sets query timeout ceiling in seconds.
func GenerateDNSUseVCAttemptsTimeoutConfig(attempts, timeoutSeconds int) string {
	if attempts <= 0 {
		attempts = 2
	}
	if timeoutSeconds <= 0 {
		timeoutSeconds = 5
	}
	return fmt.Sprintf("options use-vc attempts:%d timeout:%d\n", attempts, timeoutSeconds)
}
