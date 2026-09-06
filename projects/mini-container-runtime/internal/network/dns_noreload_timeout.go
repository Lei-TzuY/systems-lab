package network

import (
	"fmt"
)

// GenerateDNSNoReloadTimeoutConfig formats combined options no-reload timeout:N flags.
func GenerateDNSNoReloadTimeoutConfig(timeout int) string {
	if timeout <= 0 {
		timeout = 5
	}
	return fmt.Sprintf("options no-reload timeout:%d\n", timeout)
}
