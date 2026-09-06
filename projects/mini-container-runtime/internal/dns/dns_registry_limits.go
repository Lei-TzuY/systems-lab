package dns

import (
	"fmt"
	"io"
)

const (
	maxDNSRegistryBytes   int64 = 1 << 20
	maxDNSRegistryEntries       = 4096
)

func readDNSRegistryContents(r io.Reader, knownSize int64, networkName string) ([]byte, error) {
	if knownSize > maxDNSRegistryBytes {
		return nil, fmt.Errorf("DNS registry %q exceeds %d-byte limit", networkName, maxDNSRegistryBytes)
	}

	data, err := io.ReadAll(io.LimitReader(r, maxDNSRegistryBytes+1))
	if err != nil {
		return nil, fmt.Errorf("read DNS registry %q: %w", networkName, err)
	}
	if int64(len(data)) > maxDNSRegistryBytes {
		return nil, fmt.Errorf("DNS registry %q exceeds %d-byte limit", networkName, maxDNSRegistryBytes)
	}
	return data, nil
}

func validateDNSRegistryEntryCount(count int) error {
	if count > maxDNSRegistryEntries {
		return fmt.Errorf("DNS registry contains %d entries, exceeds limit %d", count, maxDNSRegistryEntries)
	}
	return nil
}
