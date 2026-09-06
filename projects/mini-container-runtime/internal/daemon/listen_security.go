package daemon

import (
	"fmt"
	"net"
)

// validateUnauthenticatedTCPAddress keeps the daemon control API local until it
// has an authentication/TLS layer. Numeric loopback addresses are deliberately
// required: wildcard, LAN, and hostname binds could expose stop/delete/inspect
// endpoints beyond the local host without any peer authentication.
func validateUnauthenticatedTCPAddress(address string) error {
	host, port, err := net.SplitHostPort(address)
	if err != nil {
		return fmt.Errorf("invalid TCP listen address %q: %w", address, err)
	}
	if host == "" {
		return fmt.Errorf("TCP listen address %q has an empty host; unauthenticated daemon requires loopback", address)
	}
	if port == "" {
		return fmt.Errorf("TCP listen address %q has an empty port", address)
	}

	ip := net.ParseIP(host)
	if ip == nil {
		return fmt.Errorf("TCP listen host %q must be a numeric loopback address while daemon authentication is unavailable", host)
	}
	if !ip.IsLoopback() {
		return fmt.Errorf("TCP listen host %q is not loopback; refusing to expose unauthenticated daemon API", host)
	}
	return nil
}
