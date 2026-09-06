package network

import (
	"fmt"
)

// GenerateIPv6Address calculates a dual-stack IPv6 address for a container interface.
func GenerateIPv6Address(index int) string {
	if index <= 0 {
		index = 2
	}
	return fmt.Sprintf("2001:db8:1::%x/64", index)
}
