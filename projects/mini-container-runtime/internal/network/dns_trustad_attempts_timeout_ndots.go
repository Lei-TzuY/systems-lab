package network

import (
	"fmt"
)

// GenerateDNSTrustADAttemptsTimeoutNdotsConfig formats combined options
// trust-ad attempts:N timeout:M ndots:K flags for /etc/resolv.conf.
//
// Non-negative values are preserved up to glibc's effective resolver caps
// (attempts=5, timeout=30s, ndots=15). Negative values request the glibc
// defaults (2, 5s, 1 respectively). In particular, zero is an explicit
// value and must not be silently rewritten to a default.
func GenerateDNSTrustADAttemptsTimeoutNdotsConfig(attempts, timeoutSeconds, ndots int) string {
	attempts = normalizeResolverOption(attempts, resolverDefaultAttempts, resolverMaxAttempts)
	timeoutSeconds = normalizeResolverOption(timeoutSeconds, resolverDefaultTimeout, resolverMaxTimeout)
	ndots = normalizeResolverOption(ndots, resolverDefaultNdots, resolverMaxNdots)

	return fmt.Sprintf("options trust-ad attempts:%d timeout:%d ndots:%d\n", attempts, timeoutSeconds, ndots)
}
