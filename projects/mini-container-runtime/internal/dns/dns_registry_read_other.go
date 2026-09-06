//go:build !linux

package dns

import (
	"fmt"
	"os"
)

func readDNSRegistryFile(path, networkName string) ([]byte, bool, error) {
	info, err := os.Lstat(path)
	if err != nil {
		if os.IsNotExist(err) {
			return nil, false, nil
		}
		return nil, false, fmt.Errorf("inspect DNS registry %q: %w", networkName, err)
	}
	if info.Mode()&os.ModeSymlink != 0 || !info.Mode().IsRegular() {
		return nil, false, fmt.Errorf("DNS registry %q must be a regular file", networkName)
	}
	file, err := os.Open(path)
	if err != nil {
		return nil, false, fmt.Errorf("open DNS registry %q: %w", networkName, err)
	}
	defer file.Close()
	data, err := readDNSRegistryContents(file, info.Size(), networkName)
	if err != nil {
		return nil, false, err
	}
	return data, true, nil
}
