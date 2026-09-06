//go:build !linux

package daemon

import "net"

func listen(network, address string) (net.Listener, error) {
	if network == "tcp" {
		if err := validateUnauthenticatedTCPAddress(address); err != nil {
			return nil, err
		}
	}
	return net.Listen(network, address)
}
