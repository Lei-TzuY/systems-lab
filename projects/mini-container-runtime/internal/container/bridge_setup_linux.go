//go:build linux

package container

import (
	"errors"
	"fmt"

	"minicontainer/internal/network"
)

type bridgeHostOps struct {
	setupVeth  func(containerPID int, hostCIDR string, debug bool) error
	removeVeth func(containerPID int, debug bool) error
	setupPort  func(hostPort, containerPort int, containerIP, protocol string, debug bool) error
	removePort func(hostPort, containerPort int, containerIP, protocol string, debug bool) error
}

func defaultBridgeHostOps(owner string) bridgeHostOps {
	hostVeth := network.VethHostIfaceOwned(owner)
	return bridgeHostOps{
		setupVeth: func(containerPID int, hostCIDR string, debug bool) error {
			return network.SetupVethHostGenerationOwned(owner, hostVeth, containerPID, hostCIDR, debug)
		},
		removeVeth: func(_ int, debug bool) error {
			return network.RemoveVethHostOwned(hostVeth, owner, debug)
		},
		setupPort: func(hostPort, containerPort int, containerIP, protocol string, debug bool) error {
			return network.SetupPortForwardingOwned(owner, hostPort, containerPort, containerIP, protocol, debug)
		},
		removePort: func(hostPort, containerPort int, containerIP, protocol string, debug bool) error {
			return network.RemovePortForwardingOwned(owner, hostPort, containerPort, containerIP, protocol, debug)
		},
	}
}

// setupBridgeHost is the compatibility wrapper for callers that do not persist
// network ownership. Without a durable recovery token it must retain eager
// rollback semantics on partial setup failure.
func setupBridgeHost(containerPID int, hostCIDR, containerIP string, mappings []PortMapping, debug bool) (func() error, error) {
	owner, err := network.NewPortForwardingOwner()
	if err != nil {
		return nil, fmt.Errorf("create bridge ownership marker: %w", err)
	}
	return setupBridgeHostWithOps(containerPID, hostCIDR, containerIP, mappings, debug, defaultBridgeHostOps(owner))
}

// setupBridgeHostOwned is used only after the managed runtime has durably
// persisted generation-scoped network ownership. It intentionally does not
// destroy successfully-created host resources on a later setup failure: the
// authoritative stopped-generation finalizer consumes that durable ownership
// only after stopped lifecycle state has committed. The returned cleanup is a
// no-op for the same reason; legacy run paths may still invoke it before state
// finalization, but managed teardown authority lives in the durable sidecar.
func setupBridgeHostOwned(containerPID int, hostCIDR, containerIP string, mappings []PortMapping, owner string, debug bool) (func() error, error) {
	if owner == "" {
		return nil, fmt.Errorf("bridge ownership marker is required")
	}
	if _, err := setupBridgeHostWithOpsPolicy(containerPID, hostCIDR, containerIP, mappings, debug, defaultBridgeHostOps(owner), false); err != nil {
		return nil, err
	}
	return func() error { return nil }, nil
}

// setupBridgeHostWithOps establishes all requested host-side bridge networking
// before the container child is released from its sync pipe. Compatibility
// callers without durable ownership get eager rollback. Managed callers use the
// policy helper below with rollbackOnFailure=false so stopped-state durability
// always precedes destructive recovery.
func setupBridgeHostWithOps(containerPID int, hostCIDR, containerIP string, mappings []PortMapping, debug bool, ops bridgeHostOps) (func() error, error) {
	return setupBridgeHostWithOpsPolicy(containerPID, hostCIDR, containerIP, mappings, debug, ops, true)
}

func setupBridgeHostWithOpsPolicy(containerPID int, hostCIDR, containerIP string, mappings []PortMapping, debug bool, ops bridgeHostOps, rollbackOnFailure bool) (func() error, error) {
	if ops.setupVeth == nil || ops.removeVeth == nil || ops.setupPort == nil || ops.removePort == nil {
		return nil, fmt.Errorf("bridge host network operation is nil")
	}

	rollbackPorts := func(installed []PortMapping) error {
		var rollbackErrs []error
		for i := len(installed) - 1; i >= 0; i-- {
			p := installed[i]
			if err := ops.removePort(p.HostPort, p.ContainerPort, containerIP, p.Protocol, debug); err != nil {
				rollbackErrs = append(rollbackErrs,
					fmt.Errorf("remove port mapping %d:%d/%s: %w", p.HostPort, p.ContainerPort, normalizedProtocol(p.Protocol), err))
			}
		}
		return errors.Join(rollbackErrs...)
	}

	cleanup := func(installed []PortMapping) error {
		portErr := rollbackPorts(installed)
		var vethErr error
		if err := ops.removeVeth(containerPID, debug); err != nil {
			vethErr = fmt.Errorf("remove host veth during bridge cleanup: %w", err)
		}
		return errors.Join(portErr, vethErr)
	}

	if err := ops.setupVeth(containerPID, hostCIDR, debug); err != nil {
		return nil, fmt.Errorf("setup host veth: %w", err)
	}

	installed := make([]PortMapping, 0, len(mappings))
	for _, p := range mappings {
		if err := ops.setupPort(p.HostPort, p.ContainerPort, containerIP, p.Protocol, debug); err != nil {
			setupErr := fmt.Errorf("setup port mapping %d:%d/%s: %w", p.HostPort, p.ContainerPort, normalizedProtocol(p.Protocol), err)
			if !rollbackOnFailure {
				return nil, setupErr
			}
			if cleanupErr := cleanup(installed); cleanupErr != nil {
				return nil, errors.Join(setupErr, cleanupErr)
			}
			return nil, setupErr
		}
		installed = append(installed, p)
	}

	return func() error { return cleanup(installed) }, nil
}

func normalizedProtocol(protocol string) string {
	if protocol == "" {
		return "tcp"
	}
	return protocol
}

type loopbackSetup func(debug bool) error
type bridgeContainerSetup func(containerCIDR, gateway string, debug bool) error

// setupBridgeContainer is the final container-side network admission gate used
// by ContainerInit before mount isolation and payload exec. ContainerInit makes
// an earlier best-effort loopback attempt for diagnostics; this gate retries the
// idempotent operation and fails closed if lo still cannot be brought up.
func setupBridgeContainer(enabled bool, containerCIDR, gateway string, debug bool) error {
	return setupContainerNetworkWith(
		enabled,
		containerCIDR,
		gateway,
		debug,
		network.SetupLoopback,
		network.SetupVethContainer,
	)
}

// setupBridgeContainerWith preserves the focused bridge-only injection surface
// used by existing tests. Production setupBridgeContainer additionally enforces
// loopback through setupContainerNetworkWith.
func setupBridgeContainerWith(enabled bool, containerCIDR, gateway string, debug bool, setup bridgeContainerSetup) error {
	return setupContainerNetworkWith(
		enabled,
		containerCIDR,
		gateway,
		debug,
		func(bool) error { return nil },
		setup,
	)
}

func setupContainerNetworkWith(enabled bool, containerCIDR, gateway string, debug bool, setupLoopback loopbackSetup, setupBridge bridgeContainerSetup) error {
	if setupLoopback == nil {
		return fmt.Errorf("container loopback network operation is nil")
	}
	if err := setupLoopback(debug); err != nil {
		return fmt.Errorf("configure container loopback: %w", err)
	}
	if !enabled {
		return nil
	}
	if setupBridge == nil {
		return fmt.Errorf("bridge container network operation is nil")
	}
	if err := setupBridge(containerCIDR, gateway, debug); err != nil {
		return fmt.Errorf("configure container bridge network: %w", err)
	}
	return nil
}
