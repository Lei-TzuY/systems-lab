package network

import (
	"crypto/sha256"
	"fmt"
)

// GenerateMACAddress generates a deterministic MAC address from container IP string.
func GenerateMACAddress(ipStr string) string {
	sum := sha256.Sum256([]byte(ipStr))
	return fmt.Sprintf("02:42:%02x:%02x:%02x:%02x", sum[0], sum[1], sum[2], sum[3])
}
