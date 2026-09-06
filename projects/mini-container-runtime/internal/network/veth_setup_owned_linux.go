//go:build linux

package network

import (
	"errors"
	"fmt"
)

type vethHostSetupOps struct {
	createPair func(name, peer string) error
	addAddr    func(name, cidr string) error
	setLinkUp  func(name string) error
	movePeer   func(name string, pid int) error
	removeHost func(pid int, debug bool) error
}

func defaultVethHostSetupOps() vethHostSetupOps {
	return vethHostSetupOps{
		createPair: createVethPair,
		addAddr:    addAddr,
		setLinkUp:  setLinkUp,
		movePeer:   moveToNetns,
		removeHost: RemoveVethHost,
	}
}

func defaultVethHostSetupOpsForOwner(owner, hostName string) vethHostSetupOps {
	return vethHostSetupOps{
		createPair: func(name, peer string) error {
			return createVethPairOwned(name, peer, owner)
		},
		addAddr:   addAddr,
		setLinkUp: setLinkUp,
		movePeer:  moveToNetns,
		removeHost: func(_ int, debug bool) error {
			return RemoveVethHostOwned(hostName, owner, debug)
		},
	}
}

// SetupVethHostOwned configures the legacy PID-named host side of a veth pair
// transactionally. Managed runtime paths should use SetupVethHostGenerationOwned
// so crash recovery can verify the exact generation before deleting the link.
func SetupVethHostOwned(containerPID int, hostCIDR string, debug bool) error {
	return setupVethHostOwnedWithOps(containerPID, hostCIDR, debug, defaultVethHostSetupOps())
}

// SetupVethHostGenerationOwned creates a generation-named veth carrying owner in
// its kernel ifalias. The durable owner/name pair can therefore be persisted
// before creation without making a failed name collision unsafe to reconcile.
func SetupVethHostGenerationOwned(owner, hostName string, containerPID int, hostCIDR string, debug bool) error {
	if err := validateGenerationNetworkOwner(owner); err != nil {
		return fmt.Errorf("validate veth owner: %w", err)
	}
	if err := validateOwnedVethName(hostName); err != nil {
		return err
	}
	if expected := VethHostIfaceOwned(owner); hostName != expected {
		return fmt.Errorf("owned veth name %q does not match generation owner (want %q)", hostName, expected)
	}
	return setupVethHostNamedWithOps(hostName, containerPID, hostCIDR, debug, defaultVethHostSetupOpsForOwner(owner, hostName))
}

func setupVethHostOwnedWithOps(containerPID int, hostCIDR string, debug bool, ops vethHostSetupOps) error {
	return setupVethHostNamedWithOps(VethHostIface(containerPID), containerPID, hostCIDR, debug, ops)
}

func setupVethHostNamedWithOps(host string, containerPID int, hostCIDR string, debug bool, ops vethHostSetupOps) error {
	if ops.createPair == nil || ops.addAddr == nil || ops.setLinkUp == nil || ops.movePeer == nil || ops.removeHost == nil {
		return fmt.Errorf("veth host setup operation is nil")
	}

	if debug {
		fmt.Printf("[parent] veth: creating pair %s ↔ %s\n", host, vethPeerName)
	}
	if err := ops.createPair(host, vethPeerName); err != nil {
		return fmt.Errorf("create veth pair: %w", err)
	}

	rollback := func(setupErr error) error {
		if cleanupErr := ops.removeHost(containerPID, debug); cleanupErr != nil {
			return errors.Join(setupErr, fmt.Errorf("rollback owned host veth: %w", cleanupErr))
		}
		return setupErr
	}

	if err := ops.addAddr(host, hostCIDR); err != nil {
		return rollback(fmt.Errorf("host addr: %w", err))
	}
	if err := ops.setLinkUp(host); err != nil {
		return rollback(fmt.Errorf("host link up: %w", err))
	}
	if err := ops.movePeer(vethPeerName, containerPID); err != nil {
		return rollback(fmt.Errorf("move peer to container netns: %w", err))
	}

	if debug {
		fmt.Printf("[parent] veth: host side %s ready (%s)\n", host, hostCIDR)
	}
	return nil
}
