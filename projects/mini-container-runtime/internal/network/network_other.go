//go:build !linux

// internal/network/network_other.go
// Non-Linux build stub for network package.

package network

import "fmt"

func SetupLoopback(debug bool) error {
	return fmt.Errorf("network namespace setup requires Linux")
}

func VethHostIface(pid int) string {
	return fmt.Sprintf("veth-h%d", pid)
}

func SetupVethHost(containerPID int, hostCIDR string, debug bool) error {
	return fmt.Errorf("veth setup requires Linux")
}

func SetupVethContainer(containerCIDR, gateway string, debug bool) error {
	return fmt.Errorf("veth container setup requires Linux")
}

func SetupNAT(subnet string, debug bool) error {
	return fmt.Errorf("NAT setup requires Linux")
}
