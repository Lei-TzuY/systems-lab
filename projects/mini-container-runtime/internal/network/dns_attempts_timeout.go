package network

import (
	"fmt"
)

// GenerateDNSAttemptsTimeoutConfig formats combined options attempts:N timeout:N flags.
func GenerateDNSAttemptsTimeoutConfig(attempts, timeout int) string {
	if attempts <= 0 {
		attempts = 2
	}
	if timeout <= 0 {
		timeout = 5
	}
	return fmt.Sprintf("options attempts:%d timeout:%d\n", attempts, timeout)
}
