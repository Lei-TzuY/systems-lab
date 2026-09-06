//go:build !linux

package dns

import "fmt"

func withDNSNetworkLock(dir, networkName string, fn func(dirFD int) error) error {
	return fmt.Errorf("cross-process DNS registry locking requires Linux")
}
