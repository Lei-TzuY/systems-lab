//go:build linux

package container

import (
	"errors"
	"fmt"

	"minicontainer/internal/dns"
	"minicontainer/internal/network"
	"minicontainer/internal/state"
)

type dnsGenerationCleanupFunc func(networkName, containerID string, pid int, pidStartTime uint64) error

func cleanupNetworkGenerationIfOwnedWith(
	st *state.Store,
	containerID string,
	pid int,
	pidStartTime uint64,
	debug bool,
	removePort ownedPortCleanupFunc,
	removeVeth ownedVethCleanupFunc,
) error {
	ownership, ok, err := st.GetNetworkOwnership(containerID)
	if err != nil {
		return fmt.Errorf("read network ownership for container %s: %w", containerID, err)
	}
	if !ok {
		return nil
	}
	if ownership.PID != pid || ownership.PIDStartTime != pidStartTime {
		// A stale lifecycle actor must never consume durable cleanup evidence
		// belonging to a newer process generation. Legacy unbound ownership is
		// intentionally left for generic stopped-state recovery.
		return nil
	}
	if err := cleanupNetworkOwnershipWith(st, containerID, ownership, debug, removePort, removeVeth); err != nil {
		return fmt.Errorf("cleanup network ownership for generation %d/%d: %w", pid, pidStartTime, err)
	}
	return nil
}

func cleanupNetworkGenerationIfOwned(st *state.Store, containerID string, pid int, pidStartTime uint64, debug bool) error {
	return cleanupNetworkGenerationIfOwnedWith(
		st,
		containerID,
		pid,
		pidStartTime,
		debug,
		network.RemovePortForwardingOwned,
		network.RemoveVethHostOwned,
	)
}

func cleanupCgroupGenerationIfOwnedWith(
	st *state.Store,
	containerID string,
	pid int,
	pidStartTime uint64,
	cleanup generationCleanupFunc,
) error {
	ownership, ok, err := st.GetCgroupOwnership(containerID)
	if err != nil {
		return fmt.Errorf("read cgroup ownership for container %s: %w", containerID, err)
	}
	if !ok {
		return nil
	}
	if ownership.PID != pid || ownership.PIDStartTime != pidStartTime {
		return nil
	}
	if err := cleanupOwnedGenerationWith(st, containerID, ownership, cleanup); err != nil {
		return fmt.Errorf("cleanup cgroup ownership for generation %d/%d: %w", pid, pidStartTime, err)
	}
	return nil
}

func cleanupRuntimeGenerationResourcesWith(
	st *state.Store,
	containerID string,
	pid int,
	pidStartTime uint64,
	cgroupCleanup generationCleanupFunc,
	removePort ownedPortCleanupFunc,
	removeVeth ownedVethCleanupFunc,
	dnsCleanup dnsGenerationCleanupFunc,
) error {
	var dnsErr error
	if dnsCleanup == nil {
		dnsErr = fmt.Errorf("DNS generation cleanup function is nil")
	} else if err := dnsCleanup(defaultBridgeDNSNetwork, containerID, pid, pidStartTime); err != nil {
		dnsErr = fmt.Errorf("cleanup bridge DNS registration for generation %d/%d: %w", pid, pidStartTime, err)
	}
	return errors.Join(
		cleanupCgroupGenerationIfOwnedWith(st, containerID, pid, pidStartTime, cgroupCleanup),
		cleanupNetworkGenerationIfOwnedWith(st, containerID, pid, pidStartTime, false, removePort, removeVeth),
		dnsErr,
	)
}

func cleanupRuntimeGenerationResources(st *state.Store, containerID string, pid int, pidStartTime uint64) error {
	return cleanupRuntimeGenerationResourcesWith(
		st,
		containerID,
		pid,
		pidStartTime,
		cleanupContainerProcessGeneration,
		network.RemovePortForwardingOwned,
		network.RemoveVethHostOwned,
		dns.CleanupStoppedHostRegistrationGeneration,
	)
}
