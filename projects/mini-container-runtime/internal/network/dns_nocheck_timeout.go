package network

import (
	"fmt"
)

// GenerateDNSNoCheckTimeoutConfig formats combined options no-check-names timeout:N flags.
func GenerateDNSNoCheckTimeoutConfig(timeout int) string {
	if timeout <= 0 {
		timeout = 5
	}
	return fmt.Sprintf("options no-check-names timeout:%d\n", timeout)
}
