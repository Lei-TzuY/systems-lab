package network

import (
	"fmt"
)

// GenerateDNSIP6BytestringAttemptsTimeoutConfig formats combined options
// ip6-bytestring attempts:N timeout:M flags for /etc/resolv.conf.
// ip6-bytestring causes IPv6 reverse lookups to use the bit-label format (RFC 2673);
// attempts sets query retries; timeout sets query timeout ceiling in seconds.
func GenerateDNSIP6BytestringAttemptsTimeoutConfig(attempts, timeoutSeconds int) string {
	if attempts <= 0 {
		attempts = 2
	}
	if timeoutSeconds <= 0 {
		timeoutSeconds = 5
	}
	return fmt.Sprintf("options ip6-bytestring attempts:%d timeout:%d\n", attempts, timeoutSeconds)
}
