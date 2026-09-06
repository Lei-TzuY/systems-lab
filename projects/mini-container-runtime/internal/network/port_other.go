//go:build !linux

// internal/network/port_other.go
// Non-Linux stub for port mapping.

package network

import "fmt"

func SetupPortForwarding(hostPort, containerPort int, containerIP, protocol string, debug bool) error {
	return fmt.Errorf("port forwarding requires Linux and iptables")
}

func RemovePortForwarding(hostPort, containerPort int, containerIP, protocol string, debug bool) error {
	return fmt.Errorf("port forwarding requires Linux and iptables")
}
