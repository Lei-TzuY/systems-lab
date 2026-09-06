package network

import (
	"fmt"
)

// GenerateDNSNoTLDAttemptsConfig formats combined options no-tld-query attempts:N flags
// for /etc/resolv.conf. no-tld-query blocks querying top-level domains for unqualified names;
// attempts sets the number of resolver retry tries before reporting failure.
func GenerateDNSNoTLDAttemptsConfig(attempts int) string {
	if attempts <= 0 {
		attempts = 2
	}
	return fmt.Sprintf("options no-tld-query attempts:%d\n", attempts)
}
