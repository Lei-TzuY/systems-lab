package network

import (
	"fmt"
)

// GenerateDNSEDNS0PayloadConfig formats options edns0 payload size flags.
func GenerateDNSEDNS0PayloadConfig(payloadSize int) string {
	if payloadSize <= 0 {
		payloadSize = 1232
	}
	return fmt.Sprintf("options edns0-payload:%d\n", payloadSize)
}
