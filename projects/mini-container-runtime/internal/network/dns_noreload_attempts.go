package network

import (
	"fmt"
)

// GenerateDNSNoReloadAttemptsConfig formats combined options no-reload attempts:N flags
// for /etc/resolv.conf. no-reload disables periodic checks for changes to /etc/resolv.conf;
// attempts sets the number of lookup attempts before reporting failure.
func GenerateDNSNoReloadAttemptsConfig(attempts int) string {
	if attempts <= 0 {
		attempts = 2
	}
	return fmt.Sprintf("options no-reload attempts:%d\n", attempts)
}
