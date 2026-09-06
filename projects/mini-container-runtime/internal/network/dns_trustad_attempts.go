package network

import (
	"fmt"
)

// GenerateDNSTrustADAttemptsConfig formats combined options trust-ad attempts:N flags
// for /etc/resolv.conf. trust-ad causes the resolver to trust the Authenticated Data (AD)
// flag in upstream DNS responses; attempts sets the retry query threshold.
func GenerateDNSTrustADAttemptsConfig(attempts int) string {
	if attempts <= 0 {
		attempts = 2
	}
	return fmt.Sprintf("options trust-ad attempts:%d\n", attempts)
}
