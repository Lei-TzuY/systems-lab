package network

import (
	"fmt"
)

// GenerateDNSSingleReqAttemptsConfig formats combined options single-request attempts:N flags
// for /etc/resolv.conf. single-request sends IPv4 and IPv6 lookups sequentially;
// attempts sets the number of query tries before giving up.
func GenerateDNSSingleReqAttemptsConfig(attempts int) string {
	if attempts <= 0 {
		attempts = 2
	}
	return fmt.Sprintf("options single-request attempts:%d\n", attempts)
}
