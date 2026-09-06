package network

import (
	"fmt"
)

// GenerateDNSSingleReqRecheckTimeoutConfig formats combined options
// single-request-recheck timeout:N for /etc/resolv.conf.
// single-request-recheck causes A and AAAA lookups to be made sequentially
// rather than in parallel, retrying the second if it times out;
// timeout sets the timeout ceiling in seconds before giving up on a nameserver query.
func GenerateDNSSingleReqRecheckTimeoutConfig(timeoutSeconds int) string {
	if timeoutSeconds <= 0 {
		timeoutSeconds = 5
	}
	return fmt.Sprintf("options single-request-recheck timeout:%d\n", timeoutSeconds)
}
