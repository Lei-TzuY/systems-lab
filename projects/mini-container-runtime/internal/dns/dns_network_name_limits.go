package dns

import "fmt"

const (
	dnsFilenameMaxBytes          = 255
	dnsRegistryTempNonceHexBytes = 16
	dnsRegistryTempFixedBytes    = len(".") + len(".json.tmp-") + dnsRegistryTempNonceHexBytes
	maxDNSNetworkNameBytes       = dnsFilenameMaxBytes - dnsRegistryTempFixedBytes
)

func maxDNSNetworkNameBytesForComponentLimit(componentLimit int64) (int, error) {
	if componentLimit <= int64(dnsRegistryTempFixedBytes) {
		return 0, fmt.Errorf("filesystem filename limit %d is too small for DNS registry temp files", componentLimit)
	}
	limit := int(componentLimit) - dnsRegistryTempFixedBytes
	if limit > maxDNSNetworkNameBytes {
		limit = maxDNSNetworkNameBytes
	}
	return limit, nil
}

func validateDNSNetworkFilenameLength(networkName string) error {
	if len(networkName) > maxDNSNetworkNameBytes {
		return fmt.Errorf("invalid network name %q: exceeds %d-byte DNS registry filename budget", networkName, maxDNSNetworkNameBytes)
	}
	return nil
}
