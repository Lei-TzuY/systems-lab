package network

import (
	"fmt"
)

// GenerateDNSInet6TimeoutConfig formats combined options inet6 timeout:N flags.
func GenerateDNSInet6TimeoutConfig(timeout int) string {
	if timeout <= 0 {
		timeout = 5
	}
	return fmt.Sprintf("options inet6 timeout:%d\n", timeout)
}
