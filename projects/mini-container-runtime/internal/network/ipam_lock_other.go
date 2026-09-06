//go:build !linux

package network

import "fmt"

func withIPAMNetworkLock(dir, networkName string, fn func() error) error {
	return fmt.Errorf("cross-process IPAM locking requires Linux")
}
