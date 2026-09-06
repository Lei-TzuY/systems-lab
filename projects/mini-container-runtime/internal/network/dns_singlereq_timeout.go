package network

import (
	"fmt"
)

// GenerateDNSSingleReqTimeoutConfig formats combined options single-request-reopen timeout:N flags.
func GenerateDNSSingleReqTimeoutConfig(timeout int) string {
	if timeout <= 0 {
		timeout = 5
	}
	return fmt.Sprintf("options single-request-reopen timeout:%d\n", timeout)
}
