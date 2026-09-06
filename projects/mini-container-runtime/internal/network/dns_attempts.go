package network

import (
	"fmt"
)

// GenerateDNSAttemptsConfig formats options attempts:N directives.
func GenerateDNSAttemptsConfig(attempts int) string {
	if attempts <= 0 {
		attempts = 2
	}
	return fmt.Sprintf("options attempts:%d\n", attempts)
}
