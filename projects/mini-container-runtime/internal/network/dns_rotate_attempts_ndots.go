package network

import (
	"fmt"
)

// GenerateDNSRotateAttemptsNDotsConfig formats combined options rotate attempts:N ndots:M flags
// for /etc/resolv.conf. rotate round-robins across nameservers; attempts sets query retries;
// ndots sets the threshold for number of dots in a name before an initial absolute query.
func GenerateDNSRotateAttemptsNDotsConfig(attempts, ndots int) string {
	if attempts <= 0 {
		attempts = 2
	}
	if ndots < 0 {
		ndots = 1
	}
	return fmt.Sprintf("options rotate attempts:%d ndots:%d\n", attempts, ndots)
}
