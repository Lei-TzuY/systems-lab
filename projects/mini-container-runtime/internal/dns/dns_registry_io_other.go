//go:build !linux

package dns

import "fmt"

func readDNSRegistryFileAt(dirFD int, name, networkName string) ([]byte, bool, error) {
	return nil, false, fmt.Errorf("dirfd-bound DNS registry reads require Linux")
}

func saveDNSRegistryFileAtomicAt(dirFD int, name, networkName string, data []byte) error {
	return fmt.Errorf("dirfd-bound DNS registry writes require Linux")
}
