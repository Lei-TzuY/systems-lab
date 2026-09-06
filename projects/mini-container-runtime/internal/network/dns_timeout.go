package network

import (
	"fmt"
)

// GenerateDNSTimeoutConfig formats options timeout:N directives.
func GenerateDNSTimeoutConfig(timeoutSeconds int) string {
	if timeoutSeconds <= 0 {
		timeoutSeconds = 2
	}
	return fmt.Sprintf("options timeout:%d\n", timeoutSeconds)
}
