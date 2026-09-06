package network

import (
	"fmt"
)

// GenerateDNSRotateAttemptsConfig formats combined options rotate attempts:N flags
// for /etc/resolv.conf. rotate causes the resolver to round-robin among nameservers;
// attempts sets the number of lookup retries before returning failure.
func GenerateDNSRotateAttemptsConfig(attempts int) string {
	if attempts <= 0 {
		attempts = 2
	}
	return fmt.Sprintf("options rotate attempts:%d\n", attempts)
}
