//go:build !linux

// internal/network/bridge_other.go
// Non-Linux build stub for bridge network management.

package network

import "fmt"

type NetworkInfo struct {
	Name   string
	Bridge string
	Subnet string
	Status string
}

func CreateBridge(_, _ string, _ bool) error {
	return fmt.Errorf("custom bridge networks require Linux iproute2")
}

func ListBridges() ([]NetworkInfo, error) {
	return nil, nil
}

func DeleteBridge(_ string, _ bool) error {
	return fmt.Errorf("custom bridge networks require Linux iproute2")
}
