package network

import (
	"fmt"
)

// GenerateDNSEDNS0SizeConfig formats options edns0-size:N directives.
func GenerateDNSEDNS0SizeConfig(bufSize int) string {
	if bufSize <= 0 {
		bufSize = 4096
	}
	return fmt.Sprintf("options edns0-size:%d\n", bufSize)
}
