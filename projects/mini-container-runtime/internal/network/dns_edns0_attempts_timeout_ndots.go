package network

import (
	"fmt"
)

// GenerateDNSEDNS0AttemptsTimeoutNdotsConfig formats combined options
// edns0 attempts:N timeout:M ndots:K flags for /etc/resolv.conf.
//
// Non-negative values are preserved up to glibc's effective resolver caps
// (attempts=5, timeout=30s, ndots=15). Negative values request the glibc
// defaults (2, 5s, 1 respectively). In particular, zero is an explicit
// value and must not be silently rewritten to a default.
func GenerateDNSEDNS0AttemptsTimeoutNdotsConfig(attempts, timeoutSeconds, ndots int) string {
	attempts = normalizeResolverOption(attempts, resolverDefaultAttempts, resolverMaxAttempts)
	timeoutSeconds = normalizeResolverOption(timeoutSeconds, resolverDefaultTimeout, resolverMaxTimeout)
	ndots = normalizeResolverOption(ndots, resolverDefaultNdots, resolverMaxNdots)

	return fmt.Sprintf("options edns0 attempts:%d timeout:%d ndots:%d\n", attempts, timeoutSeconds, ndots)
}
