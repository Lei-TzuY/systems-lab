//go:build linux

package daemon

import (
	"net"
	"sync"
	"syscall"
)

var umaskMu sync.Mutex

// listen creates Unix-domain sockets with owner-only permissions at bind time
// and refuses non-loopback TCP while the daemon has no peer authentication.
func listen(network, address string) (net.Listener, error) {
	if network == "tcp" {
		if err := validateUnauthenticatedTCPAddress(address); err != nil {
			return nil, err
		}
		return net.Listen(network, address)
	}
	if network != "unix" {
		return net.Listen(network, address)
	}

	// Unix socket mode is 0777 &^ umask, so 0177 yields 0600 without a
	// path-based chmod and its symlink-swap race. Umask is process-global;
	// serialize changes and restore it immediately after bind. Concurrent
	// unrelated file creation can at worst receive temporarily more restrictive
	// permissions.
	umaskMu.Lock()
	defer umaskMu.Unlock()
	oldMask := syscall.Umask(0o177)
	defer syscall.Umask(oldMask)
	return net.Listen(network, address)
}
