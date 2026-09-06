package network

import (
	"fmt"
)

// GenerateDNSRotateTimeoutConfig formats combined options rotate timeout:N flags.
func GenerateDNSRotateTimeoutConfig(timeout int) string {
	if timeout <= 0 {
		timeout = 5
	}
	return fmt.Sprintf("options rotate timeout:%d\n", timeout)
}
