package network

import (
	"fmt"
)

// GenerateDNSIP6DotintAttemptsTimeoutConfig formats combined options
// ip6-dotint attempts:N timeout:M flags for /etc/resolv.conf.
// ip6-dotint causes IPv6 reverse lookups to use the deprecated ip6.int zone;
// attempts sets query retries; timeout sets query timeout ceiling in seconds.
func GenerateDNSIP6DotintAttemptsTimeoutConfig(attempts, timeoutSeconds int) string {
	if attempts <= 0 {
		attempts = 2
	}
	if timeoutSeconds <= 0 {
		timeoutSeconds = 5
	}
	return fmt.Sprintf("options ip6-dotint attempts:%d timeout:%d\n", attempts, timeoutSeconds)
}
