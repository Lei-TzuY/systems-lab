package network

import (
	"fmt"
)

// GenerateDNSNoCheckAttemptsConfig formats combined options no-check-names attempts:N flags
// for /etc/resolv.conf. no-check-names disables checking of incoming host names and mail domains;
// attempts sets the number of query retries before reporting lookup failure.
func GenerateDNSNoCheckAttemptsConfig(attempts int) string {
	if attempts <= 0 {
		attempts = 2
	}
	return fmt.Sprintf("options no-check-names attempts:%d\n", attempts)
}
