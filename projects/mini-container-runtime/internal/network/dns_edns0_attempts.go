package network

import (
	"fmt"
)

// GenerateDNSEDNS0AttemptsConfig formats combined options edns0 attempts:N flags
// for /etc/resolv.conf. edns0 enables Extension Mechanisms for DNS protocol support;
// attempts sets the number of lookup attempts before failing.
func GenerateDNSEDNS0AttemptsConfig(attempts int) string {
	if attempts <= 0 {
		attempts = 2
	}
	return fmt.Sprintf("options edns0 attempts:%d\n", attempts)
}
