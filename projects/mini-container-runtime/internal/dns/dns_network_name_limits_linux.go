//go:build linux

package dns

import (
	"fmt"

	"golang.org/x/sys/unix"
)

func validateDNSNetworkFilenameLengthAt(dirFD int, networkName string) error {
	if err := validateDNSNetworkFilenameLength(networkName); err != nil {
		return err
	}
	var fs unix.Statfs_t
	if err := unix.Fstatfs(dirFD, &fs); err != nil {
		return fmt.Errorf("inspect DNS registry filesystem for %q: %w", networkName, err)
	}
	limit, err := maxDNSNetworkNameBytesForComponentLimit(fs.Namelen)
	if err != nil {
		return fmt.Errorf("DNS registry filesystem for %q: %w", networkName, err)
	}
	if len(networkName) > limit {
		return fmt.Errorf("invalid network name %q: exceeds %d-byte limit for DNS registry filenames on this filesystem", networkName, limit)
	}
	return nil
}
