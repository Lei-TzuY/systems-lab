//go:build linux

// internal/network/bridge_linux.go
//
// Custom Container Networks (`minictl network create/ls/rm`)
// ─────────────────────────────────────────────────────────
// Custom bridge networks allow multiple containers to communicate on an
// isolated virtual Layer 2 network switch (Linux Bridge interface).
//
// Commands:
//   ip link add <name> type bridge
//   ip link set dev <name> alias <ownership-tag>
//   ip addr add <subnet-gw>/24 dev <name>
//   ip link set <name> up

package network

import (
	"encoding/json"
	"errors"
	"fmt"
	"strings"
)

const (
	maxLinuxInterfaceNameLen = 15
	customBridgeAliasPrefix  = "minicontainer-network:"
)

type bridgeCommandRunner func(args ...string) ([]byte, error)

type bridgeLinkJSON struct {
	IfName  string `json:"ifname"`
	IfAlias string `json:"ifalias"`
}

// NetworkInfo describes a custom container network.
type NetworkInfo struct {
	Name   string
	Bridge string
	Subnet string
	Status string
}

func bridgeNameForNetwork(netName string) (string, error) {
	bridgeName := "br-" + netName
	if len(bridgeName) > maxLinuxInterfaceNameLen {
		return "", fmt.Errorf(
			"network name %q is too long: bridge interface %q exceeds Linux %d-byte limit",
			netName,
			bridgeName,
			maxLinuxInterfaceNameLen,
		)
	}
	return bridgeName, nil
}

func bridgeOwnershipAlias(netName string) string {
	return customBridgeAliasPrefix + netName
}

func decodeBridgeLinks(out []byte) ([]bridgeLinkJSON, error) {
	var links []bridgeLinkJSON
	if err := json.Unmarshal(out, &links); err != nil {
		return nil, fmt.Errorf("decode ip link JSON: %w", err)
	}
	return links, nil
}

// CreateBridge creates a custom Linux bridge interface with the given name and CIDR gateway.
func CreateBridge(netName, cidr string, debug bool) error {
	return createBridgeWith(netName, cidr, debug, runBridgeIPCommand)
}

func runBridgeIPCommand(args ...string) ([]byte, error) {
	return runTrustedHostTool("ip", args...)
}

func createBridgeWith(netName, cidr string, debug bool, run bridgeCommandRunner) error {
	if run == nil {
		return fmt.Errorf("bridge command runner is nil")
	}

	bridgeName, err := bridgeNameForNetwork(netName)
	if err != nil {
		return err
	}

	if cidr == "" {
		cidr = "172.28.0.1/24"
	}

	// 1. Create bridge interface.
	if out, err := run("link", "add", bridgeName, "type", "bridge"); err != nil {
		return fmt.Errorf("create bridge %s: %w\n%s", bridgeName, err, out)
	}

	// 2. Mark ownership before any additional host configuration. A later rm
	// must verify this kernel-held tag before deleting the interface.
	if out, err := run("link", "set", "dev", bridgeName, "alias", bridgeOwnershipAlias(netName)); err != nil {
		setupErr := fmt.Errorf("mark bridge %s ownership: %w\n%s", bridgeName, err, out)
		return rollbackCreatedBridge(run, bridgeName, setupErr)
	}

	// 3. Assign IP address.
	if out, err := run("addr", "add", cidr, "dev", bridgeName); err != nil {
		setupErr := fmt.Errorf("assign IP %s to %s: %w\n%s", cidr, bridgeName, err, out)
		return rollbackCreatedBridge(run, bridgeName, setupErr)
	}

	// 4. Bring bridge UP.
	if out, err := run("link", "set", bridgeName, "up"); err != nil {
		setupErr := fmt.Errorf("set %s up: %w\n%s", bridgeName, err, out)
		return rollbackCreatedBridge(run, bridgeName, setupErr)
	}

	if debug {
		fmt.Printf("[net] created custom bridge network %q (%s, %s)\n", netName, bridgeName, cidr)
	}
	return nil
}

func rollbackCreatedBridge(run bridgeCommandRunner, bridgeName string, setupErr error) error {
	out, err := run("link", "delete", bridgeName)
	if err == nil {
		return setupErr
	}
	return errors.Join(
		setupErr,
		fmt.Errorf("rollback bridge %s after setup failure: %w\n%s", bridgeName, err, out),
	)
}

// ListBridges lists only custom bridges carrying the exact minictl ownership
// tag. Merely sharing the br- prefix is not proof that a host interface belongs
// to this runtime.
func ListBridges() ([]NetworkInfo, error) {
	return listBridgesWith(runBridgeIPCommand)
}

func listBridgesWith(run bridgeCommandRunner) ([]NetworkInfo, error) {
	if run == nil {
		return nil, fmt.Errorf("bridge command runner is nil")
	}
	out, err := run("-j", "link", "show", "type", "bridge")
	if err != nil {
		return nil, fmt.Errorf("list bridge interfaces: %w\n%s", err, out)
	}
	links, err := decodeBridgeLinks(out)
	if err != nil {
		return nil, err
	}

	var networks []NetworkInfo
	for _, link := range links {
		if !strings.HasPrefix(link.IfName, "br-") {
			continue
		}
		netName := strings.TrimPrefix(link.IfName, "br-")
		canonicalName, err := bridgeNameForNetwork(netName)
		if err != nil || canonicalName != link.IfName {
			continue
		}
		if link.IfAlias != bridgeOwnershipAlias(netName) {
			continue
		}
		networks = append(networks, NetworkInfo{
			Name:   netName,
			Bridge: link.IfName,
			Status: "UP",
		})
	}
	return networks, nil
}

// DeleteBridge deletes a custom bridge network only after the kernel-held
// interface alias proves that this exact network owns the host bridge.
func DeleteBridge(netName string, debug bool) error {
	return deleteBridgeWith(netName, debug, runBridgeIPCommand)
}

func deleteBridgeWith(netName string, debug bool, run bridgeCommandRunner) error {
	if run == nil {
		return fmt.Errorf("bridge command runner is nil")
	}
	bridgeName, err := bridgeNameForNetwork(netName)
	if err != nil {
		return err
	}

	out, err := run("-j", "link", "show", "dev", bridgeName)
	if err != nil {
		return fmt.Errorf("inspect bridge %s ownership: %w\n%s", bridgeName, err, out)
	}
	links, err := decodeBridgeLinks(out)
	if err != nil {
		return fmt.Errorf("inspect bridge %s ownership: %w", bridgeName, err)
	}
	if len(links) != 1 || links[0].IfName != bridgeName {
		return fmt.Errorf("inspect bridge %s ownership: expected exactly one matching interface, got %d", bridgeName, len(links))
	}
	expectedAlias := bridgeOwnershipAlias(netName)
	if links[0].IfAlias != expectedAlias {
		return fmt.Errorf(
			"refusing to delete bridge %s without ownership tag %q (found %q)",
			bridgeName,
			expectedAlias,
			links[0].IfAlias,
		)
	}

	if out, err := run("link", "delete", bridgeName); err != nil {
		return fmt.Errorf("delete bridge %s: %w\n%s", bridgeName, err, out)
	}

	if debug {
		fmt.Printf("[net] deleted custom bridge network %q (%s)\n", netName, bridgeName)
	}
	return nil
}
