package network

import (
	"fmt"
)

// GenerateDNSUseVCAttemptsConfig formats combined options use-vc attempts:N flags
// for /etc/resolv.conf. use-vc forces TCP virtual circuits for all DNS queries;
// attempts sets the number of query retries before failure.
func GenerateDNSUseVCAttemptsConfig(attempts int) string {
	if attempts <= 0 {
		attempts = 2
	}
	return fmt.Sprintf("options use-vc attempts:%d\n", attempts)
}
