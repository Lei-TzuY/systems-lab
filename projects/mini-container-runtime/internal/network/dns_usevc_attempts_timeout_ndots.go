package network

import "fmt"

const (
	resolverDefaultAttempts = 2
	resolverDefaultTimeout  = 5
	resolverDefaultNdots    = 1
	resolverMaxAttempts     = 5
	resolverMaxTimeout      = 30
	resolverMaxNdots        = 15
)

// GenerateDNSUseVCAttemptsTimeoutNdotsConfig formats combined options
// use-vc attempts:N timeout:M ndots:K flags for /etc/resolv.conf.
//
// Non-negative values are preserved up to glibc's effective resolver caps
// (attempts=5, timeout=30s, ndots=15). Negative values request the glibc
// defaults (2, 5s, 1 respectively). In particular, zero is an explicit
// value and must not be silently rewritten to a default.
func GenerateDNSUseVCAttemptsTimeoutNdotsConfig(attempts, timeoutSeconds, ndots int) string {
	attempts = normalizeResolverOption(attempts, resolverDefaultAttempts, resolverMaxAttempts)
	timeoutSeconds = normalizeResolverOption(timeoutSeconds, resolverDefaultTimeout, resolverMaxTimeout)
	ndots = normalizeResolverOption(ndots, resolverDefaultNdots, resolverMaxNdots)

	return fmt.Sprintf("options use-vc attempts:%d timeout:%d ndots:%d\n", attempts, timeoutSeconds, ndots)
}

func normalizeResolverOption(value, defaultValue, maxValue int) int {
	if value < 0 {
		return defaultValue
	}
	if value > maxValue {
		return maxValue
	}
	return value
}
