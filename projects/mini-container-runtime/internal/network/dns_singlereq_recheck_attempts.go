package network

import (
	"fmt"
)

// GenerateDNSSingleReqRecheckAttemptsConfig formats combined options
// single-request-recheck attempts:N for /etc/resolv.conf.
// single-request-recheck causes A and AAAA lookups to be made sequentially
// rather than in parallel, retrying the second if it times out;
// attempts sets the number of query retries.
func GenerateDNSSingleReqRecheckAttemptsConfig(attempts int) string {
	if attempts <= 0 {
		attempts = 2
	}
	return fmt.Sprintf("options single-request-recheck attempts:%d\n", attempts)
}
